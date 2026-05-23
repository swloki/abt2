use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    sales_return_service_server::SalesReturnService as GrpcSalesReturnService,
    *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::server::AppState;

use abt::{CreateSalesReturnItemParams, CreateSalesReturnParams, SalesReturnService, UpdateSalesReturnParams};

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

fn status_i16_to_proto(status: i16) -> SalesReturnStatus {
    match status {
        1 => SalesReturnStatus::Pending,
        2 => SalesReturnStatus::Approved,
        3 => SalesReturnStatus::Received,
        4 => SalesReturnStatus::Completed,
        5 => SalesReturnStatus::Rejected,
        _ => SalesReturnStatus::Unspecified,
    }
}

fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or(rust_decimal::Decimal::ZERO)
}

fn decimal_to_string(d: rust_decimal::Decimal) -> String {
    d.to_string()
}

fn return_item_to_proto(item: &abt::SalesReturnItem) -> SalesReturnItem {
    SalesReturnItem {
        item_id: item.item_id,
        return_id: item.return_id,
        request_item_id: item.request_item_id,
        order_item_id: item.order_item_id,
        product_id: item.product_id,
        product_code: item.product_code.clone().unwrap_or_default(),
        product_name: item.product_name.clone().unwrap_or_default(),
        unit: item.unit.clone().unwrap_or_default(),
        unit_price: decimal_to_string(item.unit_price),
        quantity: decimal_to_string(item.quantity),
        subtotal: decimal_to_string(item.subtotal),
        remark: item.remark.clone().unwrap_or_default(),
    }
}

fn return_to_proto(r: &abt::SalesReturn) -> SalesReturn {
    SalesReturn {
        return_id: r.return_id,
        return_no: r.return_no.clone(),
        request_id: r.request_id,
        order_id: r.order_id,
        customer_name: r.customer_name.clone(),
        status: status_i16_to_proto(r.status) as i32,
        total_amount: decimal_to_string(r.total_amount),
        remark: r.remark.clone().unwrap_or_default(),
        reason: r.reason.clone().unwrap_or_default(),
        operator_id: r.operator_id.unwrap_or(0),
        created_at: r.created_at.timestamp(),
        updated_at: r.updated_at.timestamp(),
        items: r.items.iter().map(return_item_to_proto).collect(),
    }
}

fn map_create_item(i: &CreateSalesReturnItem) -> CreateSalesReturnItemParams {
    CreateSalesReturnItemParams {
        request_item_id: i.request_item_id,
        quantity: parse_decimal(&i.quantity),
        remark: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
    }
}

#[tonic::async_trait]
impl GrpcSalesReturnService for SalesReturnHandler {
    async fn create_sales_return(
        &self,
        request: Request<CreateSalesReturnRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let remark = empty_to_none(req.remark);
        let reason = empty_to_none(req.reason);

        let params = CreateSalesReturnParams {
            request_id: req.request_id,
            remark: remark.as_deref(),
            reason: reason.as_deref(),
            operator_id: None,
        };

        let items: Vec<CreateSalesReturnItemParams> = req.items.iter().map(map_create_item).collect();

        let id = srv.create(&mut tx, &params, items)
            .await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_return(
        &self,
        request: Request<UpdateSalesReturnRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let remark = empty_to_none(req.remark);
        let reason = empty_to_none(req.reason);

        let params = UpdateSalesReturnParams {
            remark: remark.as_deref(),
            reason: reason.as_deref(),
        };

        let items: Vec<CreateSalesReturnItemParams> = req.items.iter().map(map_create_item).collect();

        srv.update(&mut tx, req.return_id, &params, items)
            .await.map_err(error::err_to_status)?;

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

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(&mut tx, req.return_id).await
            .map_err(error::err_to_status)?;

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

        let r = srv.get_by_id(req.return_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("SalesReturn", &req.return_id.to_string()))?;

        Ok(Response::new(SalesReturnResponse {
            return_: Some(return_to_proto(&r)),
        }))
    }

    async fn list_sales_returns(
        &self,
        request: Request<ListSalesReturnsRequest>,
    ) -> GrpcResult<SalesReturnListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let query = abt::SalesReturnQuery {
            keyword: req.keyword,
            status: req.status.map(|s| s as i16),
            order_id: req.order_id,
            request_id: req.request_id,
            page: req.pagination.as_ref().map(|p| p.page as i64),
            page_size: req.pagination.as_ref().map(|p| p.page_size as i64),
        };

        let result = srv.list(&query).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(SalesReturnListResponse {
            items: result.items.iter().map(return_to_proto).collect(),
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

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let new_status = req.status as i16;

        srv.update_status(&mut tx, req.return_id, new_status).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
