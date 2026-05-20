//! Quotation gRPC Handler

use crate::generated::abt::v1::{
    quotation_service_server::QuotationService as GrpcQuotationService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use common::error;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tonic::{Request, Response};

use abt::QuotationService;

pub struct QuotationHandler;

impl QuotationHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for QuotationHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn status_i16_to_proto(status: i16) -> i32 {
    match status {
        1 => QuotationStatus::Draft as i32,
        2 => QuotationStatus::Submitted as i32,
        3 => QuotationStatus::Accepted as i32,
        4 => QuotationStatus::Rejected as i32,
        5 => QuotationStatus::Expired as i32,
        _ => QuotationStatus::Unspecified as i32,
    }
}

fn status_proto_to_i16(status: i32) -> i16 {
    match status {
        1 => 1, // Draft
        2 => 2, // Submitted
        3 => 3, // Accepted
        4 => 4, // Rejected
        5 => 5, // Expired
        _ => 0,
    }
}

fn proto_item_to_model(item: &CreateQuotationItem, quotation_id: i64) -> abt::QuotationItem {
    abt::QuotationItem {
        item_id: 0,
        quotation_id,
        product_id: item.product_id,
        product_code: None,
        product_name: None,
        unit: None,
        unit_price: Decimal::from_f64_retain(item.unit_price).unwrap_or(Decimal::ZERO),
        quantity: Decimal::from_f64_retain(item.quantity).unwrap_or(Decimal::ZERO),
        discount: Decimal::from_f64_retain(item.discount).unwrap_or(Decimal::ONE),
        subtotal: Decimal::ZERO, // calculated in service
        remark: if item.remark.is_empty() {
            None
        } else {
            Some(item.remark.clone())
        },
        created_at: chrono::NaiveDateTime::default(),
    }
}

fn quotation_item_to_proto(item: abt::QuotationItem) -> QuotationItem {
    QuotationItem {
        item_id: item.item_id,
        quotation_id: item.quotation_id,
        product_id: item.product_id,
        product_code: item.product_code.unwrap_or_default(),
        product_name: item.product_name.unwrap_or_default(),
        unit: item.unit.unwrap_or_default(),
        unit_price: item.unit_price.to_f64().unwrap_or(0.0),
        quantity: item.quantity.to_f64().unwrap_or(0.0),
        discount: item.discount.to_f64().unwrap_or(1.0),
        subtotal: item.subtotal.to_f64().unwrap_or(0.0),
        remark: item.remark.unwrap_or_default(),
    }
}

fn quotation_to_proto(q: abt::Quotation) -> Quotation {
    Quotation {
        quotation_id: q.quotation_id,
        quotation_no: q.quotation_no,
        customer_name: q.customer_name,
        contact_person: q.contact_person.unwrap_or_default(),
        contact_phone: q.contact_phone.unwrap_or_default(),
        status: status_i16_to_proto(q.status),
        total_amount: q.total_amount.to_f64().unwrap_or(0.0),
        remark: q.remark.unwrap_or_default(),
        valid_until: q
            .valid_until
            .map(|dt| dt.and_utc().timestamp())
            .unwrap_or(0),
        created_at: q.created_at.and_utc().timestamp(),
        updated_at: q.updated_at.and_utc().timestamp(),
        operator_id: q.operator_id.unwrap_or(0),
        items: q.items.into_iter().map(quotation_item_to_proto).collect(),
    }
}

#[tonic::async_trait]
impl GrpcQuotationService for QuotationHandler {
    async fn create_quotation(
        &self,
        request: Request<CreateQuotationRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let items: Vec<abt::QuotationItem> = req.items.iter().map(|i| proto_item_to_model(i, 0)).collect();

        let quotation = abt::Quotation {
            quotation_id: 0,
            quotation_no: String::new(), // generated in service
            customer_name: req.customer_name,
            contact_person: if req.contact_person.is_empty() {
                None
            } else {
                Some(req.contact_person)
            },
            contact_phone: if req.contact_phone.is_empty() {
                None
            } else {
                Some(req.contact_phone)
            },
            status: 1, // Draft
            total_amount: Decimal::ZERO, // calculated in service
            remark: if req.remark.is_empty() {
                None
            } else {
                Some(req.remark)
            },
            valid_until: if req.valid_until > 0 {
                chrono::DateTime::from_timestamp(req.valid_until, 0)
                    .map(|dt| dt.naive_utc())
            } else {
                None
            },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        let id = srv
            .create(Some(auth.user_id), quotation, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_quotation(
        &self,
        request: Request<UpdateQuotationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let items: Vec<abt::QuotationItem> = req
            .items
            .iter()
            .map(|i| proto_item_to_model(i, req.quotation_id))
            .collect();

        let quotation = abt::Quotation {
            quotation_id: req.quotation_id,
            quotation_no: String::new(), // not updated
            customer_name: req.customer_name,
            contact_person: if req.contact_person.is_empty() {
                None
            } else {
                Some(req.contact_person)
            },
            contact_phone: if req.contact_phone.is_empty() {
                None
            } else {
                Some(req.contact_phone)
            },
            status: 0, // not updated
            total_amount: Decimal::ZERO, // calculated in service
            remark: if req.remark.is_empty() {
                None
            } else {
                Some(req.remark)
            },
            valid_until: if req.valid_until > 0 {
                chrono::DateTime::from_timestamp(req.valid_until, 0)
                    .map(|dt| dt.naive_utc())
            } else {
                None
            },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        srv.update(Some(auth.user_id), quotation, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_quotation(
        &self,
        request: Request<DeleteQuotationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete(req.quotation_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_quotation(
        &self,
        request: Request<GetQuotationRequest>,
    ) -> GrpcResult<QuotationResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let quotation = srv
            .get_by_id(req.quotation_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| {
                error::not_found("Quotation", &req.quotation_id.to_string())
            })?;

        Ok(Response::new(QuotationResponse {
            quotation: Some(quotation_to_proto(quotation)),
        }))
    }

    async fn list_quotations(
        &self,
        request: Request<ListQuotationsRequest>,
    ) -> GrpcResult<QuotationListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let pagination = req.pagination.unwrap_or(PaginationParams {
            page: 1,
            page_size: 20,
        });

        let query = abt::QuotationQuery {
            keyword: req.keyword,
            status: req.status.map(status_proto_to_i16),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(QuotationListResponse {
            items: result
                .items
                .into_iter()
                .map(quotation_to_proto)
                .collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    async fn update_quotation_status(
        &self,
        request: Request<UpdateQuotationStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let status = status_proto_to_i16(req.status);

        srv.update_status(req.quotation_id, status, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
