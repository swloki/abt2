use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    quotation_service_server::QuotationService as GrpcQuotationService,
    *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::server::AppState;

use abt::{CreateQuotationItemParams, CreateQuotationParams, QuotationService, UpdateQuotationParams};

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

fn status_i16_to_proto(status: i16) -> QuotationStatus {
    match status {
        1 => QuotationStatus::Draft,
        2 => QuotationStatus::Submitted,
        3 => QuotationStatus::Accepted,
        4 => QuotationStatus::Rejected,
        5 => QuotationStatus::Expired,
        _ => QuotationStatus::Unspecified,
    }
}

fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or(rust_decimal::Decimal::ZERO)
}

fn decimal_to_string(d: rust_decimal::Decimal) -> String {
    d.to_string()
}

fn quotation_item_to_proto(item: &abt::QuotationItem) -> QuotationItem {
    QuotationItem {
        item_id: item.item_id,
        quotation_id: item.quotation_id,
        product_id: item.product_id,
        product_code: item.product_code.clone().unwrap_or_default(),
        product_name: item.product_name.clone().unwrap_or_default(),
        unit: item.unit.clone().unwrap_or_default(),
        unit_price: decimal_to_string(item.unit_price),
        quantity: decimal_to_string(item.quantity),
        discount: decimal_to_string(item.discount),
        subtotal: decimal_to_string(item.subtotal),
        remark: item.remark.clone().unwrap_or_default(),
    }
}

fn quotation_to_proto(q: &abt::Quotation) -> Quotation {
    Quotation {
        quotation_id: q.quotation_id,
        quotation_no: q.quotation_no.clone(),
        customer_name: q.customer_name.clone(),
        contact_person: q.contact_person.clone().unwrap_or_default(),
        contact_phone: q.contact_phone.clone().unwrap_or_default(),
        status: status_i16_to_proto(q.status) as i32,
        total_amount: decimal_to_string(q.total_amount),
        remark: q.remark.clone().unwrap_or_default(),
        valid_until: q.valid_until.map(|d| d.timestamp()).unwrap_or(0),
        created_at: q.created_at.timestamp(),
        updated_at: q.updated_at.timestamp(),
        operator_id: q.operator_id.unwrap_or(0),
        items: q.items.iter().map(quotation_item_to_proto).collect(),
    }
}

fn map_item(i: &CreateQuotationItem) -> CreateQuotationItemParams {
    CreateQuotationItemParams {
        product_id: i.product_id,
        product_code: None,
        product_name: None,
        unit: None,
        unit_price: parse_decimal(&i.unit_price),
        quantity: parse_decimal(&i.quantity),
        discount: parse_decimal(&i.discount),
        remark: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
    }
}

#[tonic::async_trait]
impl GrpcQuotationService for QuotationHandler {
    async fn create_quotation(
        &self,
        request: Request<CreateQuotationRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let valid_until = if req.valid_until > 0 {
            Some(chrono::DateTime::from_timestamp(req.valid_until, 0)
                .unwrap_or(chrono::Utc::now()))
        } else {
            None
        };

        let contact_person = empty_to_none(req.contact_person);
        let contact_phone = empty_to_none(req.contact_phone);
        let remark = empty_to_none(req.remark);

        let params = CreateQuotationParams {
            customer_name: &req.customer_name,
            contact_person: contact_person.as_deref(),
            contact_phone: contact_phone.as_deref(),
            remark: remark.as_deref(),
            valid_until,
            operator_id: None,
        };

        let items: Vec<CreateQuotationItemParams> = req.items.iter().map(map_item).collect();

        let id = srv.create(&mut tx, &params, items)
            .await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_quotation(
        &self,
        request: Request<UpdateQuotationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let valid_until = if req.valid_until > 0 {
            Some(chrono::DateTime::from_timestamp(req.valid_until, 0)
                .unwrap_or(chrono::Utc::now()))
        } else {
            None
        };

        let contact_person = empty_to_none(req.contact_person);
        let contact_phone = empty_to_none(req.contact_phone);
        let remark = empty_to_none(req.remark);

        let params = UpdateQuotationParams {
            customer_name: &req.customer_name,
            contact_person: contact_person.as_deref(),
            contact_phone: contact_phone.as_deref(),
            remark: remark.as_deref(),
            valid_until,
        };

        let items: Vec<CreateQuotationItemParams> = req.items.iter().map(map_item).collect();

        srv.update(&mut tx, req.quotation_id, &params, items)
            .await.map_err(error::err_to_status)?;

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

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(&mut tx, req.quotation_id).await
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

        let q = srv.get_by_id(req.quotation_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Quotation", &req.quotation_id.to_string()))?;

        Ok(Response::new(QuotationResponse {
            quotation: Some(quotation_to_proto(&q)),
        }))
    }

    async fn list_quotations(
        &self,
        request: Request<ListQuotationsRequest>,
    ) -> GrpcResult<QuotationListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_service();

        let query = abt::QuotationQuery {
            keyword: req.keyword,
            status: req.status.map(|s| s as i16),
            page: req.pagination.as_ref().map(|p| p.page as i64),
            page_size: req.pagination.as_ref().map(|p| p.page_size as i64),
        };

        let result = srv.list(&query).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(QuotationListResponse {
            items: result.items.iter().map(quotation_to_proto).collect(),
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

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let new_status = req.status as i16;

        srv.update_status(&mut tx, req.quotation_id, new_status).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
