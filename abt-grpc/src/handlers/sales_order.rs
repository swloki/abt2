use crate::generated::abt::v1::{
    sales_order_service_server::SalesOrderService as GrpcSalesOrderService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use common::error;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tonic::{Request, Response};

use abt::SalesOrderService;

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

fn status_i16_to_proto(status: i16) -> i32 {
    match status {
        1 => SalesOrderStatus::Draft as i32,
        2 => SalesOrderStatus::Confirmed as i32,
        3 => SalesOrderStatus::InProgress as i32,
        4 => SalesOrderStatus::Completed as i32,
        5 => SalesOrderStatus::Cancelled as i32,
        _ => SalesOrderStatus::Unspecified as i32,
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

fn proto_item_to_model(item: &CreateSalesOrderItem, order_id: i64) -> abt::SalesOrderItem {
    abt::SalesOrderItem {
        item_id: 0,
        order_id,
        product_id: item.product_id,
        product_code: None,
        product_name: None,
        unit: None,
        unit_price: Decimal::from_f64_retain(item.unit_price).unwrap_or(Decimal::ZERO),
        quantity: Decimal::from_f64_retain(item.quantity).unwrap_or(Decimal::ZERO),
        discount: Decimal::from_f64_retain(item.discount).unwrap_or(Decimal::ONE),
        subtotal: Decimal::ZERO,
        shipped_qty: Decimal::ZERO,
        returned_qty: Decimal::ZERO,
        remark: if item.remark.is_empty() { None } else { Some(item.remark.clone()) },
        created_at: chrono::NaiveDateTime::default(),
    }
}

fn order_item_to_proto(item: abt::SalesOrderItem) -> SalesOrderItem {
    SalesOrderItem {
        item_id: item.item_id,
        order_id: item.order_id,
        product_id: item.product_id,
        product_code: item.product_code.unwrap_or_default(),
        product_name: item.product_name.unwrap_or_default(),
        unit: item.unit.unwrap_or_default(),
        unit_price: item.unit_price.to_f64().unwrap_or(0.0),
        quantity: item.quantity.to_f64().unwrap_or(0.0),
        discount: item.discount.to_f64().unwrap_or(1.0),
        subtotal: item.subtotal.to_f64().unwrap_or(0.0),
        shipped_qty: item.shipped_qty.to_f64().unwrap_or(0.0),
        returned_qty: item.returned_qty.to_f64().unwrap_or(0.0),
        remark: item.remark.unwrap_or_default(),
    }
}

fn sales_order_to_proto(o: abt::SalesOrder) -> SalesOrder {
    SalesOrder {
        order_id: o.order_id,
        order_no: o.order_no,
        quotation_id: o.quotation_id.unwrap_or(0),
        customer_name: o.customer_name,
        contact_person: o.contact_person.unwrap_or_default(),
        contact_phone: o.contact_phone.unwrap_or_default(),
        status: status_i16_to_proto(o.status),
        total_amount: o.total_amount.to_f64().unwrap_or(0.0),
        remark: o.remark.unwrap_or_default(),
        delivery_date: o.delivery_date.map(|dt| dt.and_utc().timestamp()).unwrap_or(0),
        operator_id: o.operator_id.unwrap_or(0),
        created_at: o.created_at.and_utc().timestamp(),
        updated_at: o.updated_at.and_utc().timestamp(),
        items: o.items.into_iter().map(order_item_to_proto).collect(),
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
        let srv = state.sales_order_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items: Vec<abt::SalesOrderItem> = req.items.iter().map(|i| proto_item_to_model(i, 0)).collect();

        let order = abt::SalesOrder {
            order_id: 0,
            order_no: String::new(),
            quotation_id: if req.quotation_id > 0 { Some(req.quotation_id) } else { None },
            customer_name: req.customer_name,
            contact_person: if req.contact_person.is_empty() { None } else { Some(req.contact_person) },
            contact_phone: if req.contact_phone.is_empty() { None } else { Some(req.contact_phone) },
            status: 0,
            total_amount: Decimal::ZERO,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            delivery_date: if req.delivery_date > 0 {
                chrono::DateTime::from_timestamp(req.delivery_date, 0).map(|dt| dt.naive_utc())
            } else {
                None
            },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        let id = srv.create(Some(auth.user_id), order, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_order(
        &self,
        request: Request<UpdateSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        srv.update_header(
            req.order_id,
            req.customer_name,
            if req.contact_person.is_empty() { None } else { Some(req.contact_person) },
            if req.contact_phone.is_empty() { None } else { Some(req.contact_phone) },
            if req.remark.is_empty() { None } else { Some(req.remark) },
            if req.delivery_date > 0 {
                chrono::DateTime::from_timestamp(req.delivery_date, 0).map(|dt| dt.naive_utc())
            } else {
                None
            },
        ).await.map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_sales_order(
        &self,
        request: Request<DeleteSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        srv.delete(req.order_id, &mut tx).await.map_err(error::err_to_status)?;
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

        let order = srv.get_by_id(req.order_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("SalesOrder", &req.order_id.to_string()))?;

        Ok(Response::new(SalesOrderResponse {
            order: Some(sales_order_to_proto(order)),
        }))
    }

    async fn list_sales_orders(
        &self,
        request: Request<ListSalesOrdersRequest>,
    ) -> GrpcResult<SalesOrderListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::SalesOrderQuery {
            keyword: req.keyword,
            status: req.status.map(status_proto_to_i16),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(SalesOrderListResponse {
            items: result.items.into_iter().map(sales_order_to_proto).collect(),
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

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        let status = status_proto_to_i16(req.status);
        srv.update_status(req.order_id, status, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
