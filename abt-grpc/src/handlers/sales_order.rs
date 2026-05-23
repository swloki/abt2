use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    sales_order_service_server::SalesOrderService as GrpcSalesOrderService,
    *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::server::AppState;

use abt::{CreateSalesOrderItemParams, CreateSalesOrderParams, SalesOrderService, UpdateSalesOrderHeaderParams};

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

fn status_i16_to_proto(status: i16) -> SalesOrderStatus {
    match status {
        1 => SalesOrderStatus::Draft,
        2 => SalesOrderStatus::Confirmed,
        3 => SalesOrderStatus::InProgress,
        4 => SalesOrderStatus::Completed,
        5 => SalesOrderStatus::Cancelled,
        _ => SalesOrderStatus::Unspecified,
    }
}

fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or(rust_decimal::Decimal::ZERO)
}

fn decimal_to_string(d: rust_decimal::Decimal) -> String {
    d.to_string()
}

fn order_item_to_proto(item: &abt::SalesOrderItem) -> SalesOrderItem {
    SalesOrderItem {
        item_id: item.item_id,
        order_id: item.order_id,
        product_id: item.product_id,
        product_code: item.product_code.clone().unwrap_or_default(),
        product_name: item.product_name.clone().unwrap_or_default(),
        unit: item.unit.clone().unwrap_or_default(),
        unit_price: decimal_to_string(item.unit_price),
        quantity: decimal_to_string(item.quantity),
        discount: decimal_to_string(item.discount),
        subtotal: decimal_to_string(item.subtotal),
        shipped_qty: decimal_to_string(item.shipped_qty),
        returned_qty: decimal_to_string(item.returned_qty),
        remark: item.remark.clone().unwrap_or_default(),
    }
}

fn order_to_proto(o: &abt::SalesOrder) -> SalesOrder {
    SalesOrder {
        order_id: o.order_id,
        order_no: o.order_no.clone(),
        quotation_id: o.quotation_id.unwrap_or(0),
        customer_name: o.customer_name.clone(),
        contact_person: o.contact_person.clone().unwrap_or_default(),
        contact_phone: o.contact_phone.clone().unwrap_or_default(),
        status: status_i16_to_proto(o.status) as i32,
        total_amount: decimal_to_string(o.total_amount),
        remark: o.remark.clone().unwrap_or_default(),
        delivery_date: o.delivery_date.map(|d| d.timestamp()).unwrap_or(0),
        created_at: o.created_at.timestamp(),
        updated_at: o.updated_at.timestamp(),
        operator_id: o.operator_id.unwrap_or(0),
        items: o.items.iter().map(order_item_to_proto).collect(),
    }
}

fn map_create_item(i: &CreateSalesOrderItem) -> CreateSalesOrderItemParams {
    CreateSalesOrderItemParams {
        product_id: i.product_id,
        unit_price: parse_decimal(&i.unit_price),
        quantity: parse_decimal(&i.quantity),
        discount: parse_decimal(&i.discount),
        remark: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
    }
}

#[tonic::async_trait]
impl GrpcSalesOrderService for SalesOrderHandler {
    async fn create_sales_order(
        &self,
        request: Request<CreateSalesOrderRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let delivery_date = if req.delivery_date > 0 {
            Some(chrono::DateTime::from_timestamp(req.delivery_date, 0)
                .unwrap_or(chrono::Utc::now()))
        } else {
            None
        };

        let contact_person = empty_to_none(req.contact_person);
        let contact_phone = empty_to_none(req.contact_phone);
        let remark = empty_to_none(req.remark);

        let quotation_id = if req.quotation_id > 0 { Some(req.quotation_id) } else { None };

        let params = CreateSalesOrderParams {
            quotation_id,
            customer_name: &req.customer_name,
            contact_person: contact_person.as_deref(),
            contact_phone: contact_phone.as_deref(),
            remark: remark.as_deref(),
            delivery_date,
            operator_id: None,
        };

        let items: Vec<CreateSalesOrderItemParams> = req.items.iter().map(map_create_item).collect();

        let id = srv.create(&mut tx, &params, items)
            .await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_order(
        &self,
        request: Request<UpdateSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let delivery_date = if req.delivery_date > 0 {
            Some(chrono::DateTime::from_timestamp(req.delivery_date, 0)
                .unwrap_or(chrono::Utc::now()))
        } else {
            None
        };

        let contact_person = empty_to_none(req.contact_person);
        let contact_phone = empty_to_none(req.contact_phone);
        let remark = empty_to_none(req.remark);

        let params = UpdateSalesOrderHeaderParams {
            customer_name: &req.customer_name,
            contact_person: contact_person.as_deref(),
            contact_phone: contact_phone.as_deref(),
            remark: remark.as_deref(),
            delivery_date,
        };

        srv.update_header(&mut tx, req.order_id, &params)
            .await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_sales_order(
        &self,
        request: Request<DeleteSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(&mut tx, req.order_id).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_sales_order(
        &self,
        request: Request<GetSalesOrderRequest>,
    ) -> GrpcResult<SalesOrderResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let o = srv.get_by_id(req.order_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("SalesOrder", &req.order_id.to_string()))?;

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
        let srv = state.sales_order_service();

        let query = abt::SalesOrderQuery {
            keyword: req.keyword,
            status: req.status.map(|s| s as i16),
            page: req.pagination.as_ref().map(|p| p.page as i64),
            page_size: req.pagination.as_ref().map(|p| p.page_size as i64),
        };

        let result = srv.list(&query).await
            .map_err(error::err_to_status)?;

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
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let new_status = req.status as i16;

        srv.update_status(&mut tx, req.order_id, new_status).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
