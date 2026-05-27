//! SalesReturn gRPC Handler — 委托给 abt-core SalesReturnService

use abt_core::sales::sales_return::SalesReturnService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    sales_return_service_server::SalesReturnService as GrpcSalesReturnService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

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

fn return_status_to_proto(status: abt_core::sales::sales_return::ReturnStatus) -> SalesReturnStatus {
    match status {
        abt_core::sales::sales_return::ReturnStatus::Draft => SalesReturnStatus::Draft,
        abt_core::sales::sales_return::ReturnStatus::Confirmed => SalesReturnStatus::Confirmed,
        abt_core::sales::sales_return::ReturnStatus::Received => SalesReturnStatus::Received,
        abt_core::sales::sales_return::ReturnStatus::Inspecting => SalesReturnStatus::Inspecting,
        abt_core::sales::sales_return::ReturnStatus::Completed => SalesReturnStatus::Completed,
        abt_core::sales::sales_return::ReturnStatus::Cancelled => SalesReturnStatus::Cancelled,
        abt_core::sales::sales_return::ReturnStatus::Rejected => SalesReturnStatus::Rejected,
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
fn return_item_to_proto(item: &abt_core::sales::sales_return::SalesReturnItem) -> SalesReturnItem {
    SalesReturnItem {
        item_id: item.id,
        return_id: item.return_id,
        request_item_id: 0,
        order_item_id: item.order_item_id,
        product_id: item.product_id,
        product_code: String::new(),
        product_name: String::new(),
        unit: String::new(),
        unit_price: decimal_to_string(item.unit_price),
        quantity: decimal_to_string(item.returned_qty),
        subtotal: decimal_to_string(item.amount),
        remark: String::new(),
        disposition: 0,
    }
}

fn return_to_proto(r: &abt_core::sales::sales_return::SalesReturn) -> SalesReturn {
    SalesReturn {
        return_id: r.id,
        return_no: r.doc_number.clone(),
        request_id: r.shipping_request_id,
        order_id: r.order_id,
        customer_name: String::new(),
        status: return_status_to_proto(r.status) as i32,
        total_amount: decimal_to_string(r.total_amount),
        remark: r.remark.clone(),
        reason: r.return_reason.clone(),
        operator_id: r.operator_id,
        created_at: r.created_at.timestamp(),
        updated_at: r.updated_at.timestamp(),
        items: vec![],
        customer_id: 0,
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
        let srv = state.sales_return_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let items: Vec<abt_core::sales::sales_return::CreateReturnItemReq> = req.items.iter().map(|i| {
            abt_core::sales::sales_return::CreateReturnItemReq {
                order_item_id: i.request_item_id,
                returned_qty: parse_decimal(&i.quantity),
                disposition: abt_core::sales::sales_return::ReturnDisposition::Restock,
            }
        }).collect();

        let create_req = abt_core::sales::sales_return::CreateReturnReq {
            order_id: 0, // TODO: proto 目前没有 order_id 字段，需要适配
            shipping_request_id: req.request_id,
            customer_id: 0,
            return_reason: req.reason.clone(),
            items,
        };

        let id = srv.create(ctx, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_return(
        &self,
        request: Request<UpdateSalesReturnRequest>,
    ) -> GrpcResult<BoolResponse> {
        // abt-core SalesReturnService 没有通用 update 方法
        let _ = request;
        Err(tonic::Status::unimplemented("Update sales return not supported in abt-core; use status transitions"))
    }

    async fn delete_sales_return(
        &self,
        request: Request<DeleteSalesReturnRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _ = request;
        // abt-core SalesReturnService 没有 delete 方法
        Err(tonic::Status::unimplemented("Delete sales return not supported in abt-core"))
    }

    async fn get_sales_return(
        &self,
        request: Request<GetSalesReturnRequest>,
    ) -> GrpcResult<SalesReturnResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let r = srv.find_by_id(ctx, req.return_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(SalesReturnResponse {
            r#return: Some(return_to_proto(&r)),
        }))
    }

    async fn list_sales_returns(
        &self,
        request: Request<ListSalesReturnsRequest>,
    ) -> GrpcResult<SalesReturnListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);

        let filter = abt_core::sales::sales_return::ReturnQuery {
            order_id: req.order_id,
            shipping_request_id: req.request_id,
            customer_id: None,
            status: req.status.and_then(|s| {
                abt_core::sales::sales_return::ReturnStatus::from_i16(s as i16)
            }),
            keyword: req.keyword,
        };
        let page = PageParams::new(
            req.pagination.as_ref().map(|p| p.page).unwrap_or(1),
            req.pagination.as_ref().map(|p| p.page_size).unwrap_or(20),
        );

        let result = srv.list(ctx, filter, page).await
            .map_err(domain_to_status)?;

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
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        match req.status() {
            SalesReturnStatus::Confirmed => {
                srv.approve(ctx, req.return_id).await
                    .map_err(domain_to_status)?;
            }
            SalesReturnStatus::Received => {
                srv.receive(ctx, req.return_id).await
                    .map_err(domain_to_status)?;
            }
            SalesReturnStatus::Completed => {
                srv.complete(ctx, req.return_id).await
                    .map_err(domain_to_status)?;
            }
            SalesReturnStatus::Rejected => {
                srv.reject(ctx, req.return_id).await
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
