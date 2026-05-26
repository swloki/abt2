//! Quotation gRPC Handler — 委托给 abt-core QuotationService

use abt_core::sales::quotation::QuotationService;
use abt_core::shared::types::{PageParams, ServiceContext};
use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    quotation_service_server::QuotationService as GrpcQuotationService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

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

fn quotation_status_to_proto(status: abt_core::sales::quotation::QuotationStatus) -> QuotationStatus {
    match status {
        abt_core::sales::quotation::QuotationStatus::Draft => QuotationStatus::Draft,
        abt_core::sales::quotation::QuotationStatus::Sent => QuotationStatus::Submitted,
        abt_core::sales::quotation::QuotationStatus::Accepted => QuotationStatus::Accepted,
        abt_core::sales::quotation::QuotationStatus::Rejected => QuotationStatus::Rejected,
        abt_core::sales::quotation::QuotationStatus::Expired => QuotationStatus::Expired,
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
fn quotation_item_to_proto(item: &abt_core::sales::quotation::QuotationItem) -> QuotationItem {
    QuotationItem {
        item_id: item.id,
        quotation_id: item.quotation_id,
        product_id: item.product_id,
        product_code: String::new(),
        product_name: item.description.clone(),
        unit: item.unit.clone(),
        unit_price: decimal_to_string(item.unit_price),
        quantity: decimal_to_string(item.quantity),
        discount: decimal_to_string(item.discount_rate),
        subtotal: decimal_to_string(item.amount),
        remark: String::new(),
    }
}

fn quotation_to_proto(q: &abt_core::sales::quotation::Quotation) -> Quotation {
    Quotation {
        quotation_id: q.id,
        quotation_no: q.doc_number.clone(),
        customer_name: String::new(), // customer_id 需要额外查询
        contact_person: String::new(),
        contact_phone: String::new(),
        status: quotation_status_to_proto(q.status) as i32,
        total_amount: decimal_to_string(q.total_amount),
        remark: q.remark.clone(),
        valid_until: q.valid_until.and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .and_utc().timestamp(),
        created_at: q.created_at.timestamp(),
        updated_at: q.updated_at.timestamp(),
        operator_id: q.operator_id,
        items: vec![], // items 需要额外查询
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
        let srv = state.quotation_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let valid_until = if req.valid_until > 0 {
            chrono::DateTime::from_timestamp(req.valid_until, 0)
                .map(|dt| dt.date_naive())
                .unwrap_or(chrono::Utc::now().date_naive())
        } else {
            chrono::Utc::now().date_naive()
        };

        let items: Vec<abt_core::sales::quotation::CreateQuotationItemReq> = req.items.iter().map(|i| {
            abt_core::sales::quotation::CreateQuotationItemReq {
                product_id: i.product_id,
                description: None,
                quantity: parse_decimal(&i.quantity),
                unit: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
                unit_price: parse_decimal(&i.unit_price),
                unit_cost: None,
                discount_rate: {
                    let d = parse_decimal(&i.discount);
                    if d == rust_decimal::Decimal::ZERO { None } else { Some(d) }
                },
                delivery_date: None,
            }
        }).collect();

        let create_req = abt_core::sales::quotation::CreateQuotationReq {
            customer_id: 0, // TODO: proto 目前只有 customer_name，需要适配
            contact_id: 0,
            valid_until,
            items,
            payment_terms: None,
            delivery_terms: None,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        let id = srv.create(ctx, create_req).await
            .map_err(domain_to_status)?;

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
        let srv = state.quotation_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let valid_until = if req.valid_until > 0 {
            chrono::DateTime::from_timestamp(req.valid_until, 0)
                .map(|dt| dt.date_naive())
                .map(Some)
                .unwrap_or(None)
        } else {
            None
        };

        let items: Vec<abt_core::sales::quotation::CreateQuotationItemReq> = req.items.iter().map(|i| {
            abt_core::sales::quotation::CreateQuotationItemReq {
                product_id: i.product_id,
                description: None,
                quantity: parse_decimal(&i.quantity),
                unit: None,
                unit_price: parse_decimal(&i.unit_price),
                unit_cost: None,
                discount_rate: {
                    let d = parse_decimal(&i.discount);
                    if d == rust_decimal::Decimal::ZERO { None } else { Some(d) }
                },
                delivery_date: None,
            }
        }).collect();

        let update_req = abt_core::sales::quotation::UpdateQuotationReq {
            customer_id: None,
            contact_id: None,
            sales_rep_id: None,
            valid_until,
            payment_terms: None,
            delivery_terms: None,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            items: Some(items),
        };

        srv.update(ctx, req.quotation_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_quotation(
        &self,
        request: Request<DeleteQuotationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        // abt-core QuotationService 没有 delete 方法，返回不支持
        drop(ctx);
        drop(srv);
        drop(tx);

        let _ = (req, auth);
        Err(tonic::Status::unimplemented("Delete quotation not supported in abt-core"))
    }

    async fn get_quotation(
        &self,
        request: Request<GetQuotationRequest>,
    ) -> GrpcResult<QuotationResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let q = srv.find_by_id(ctx, req.quotation_id).await
            .map_err(domain_to_status)?;

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
        let srv = state.quotation_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);

        let filter = abt_core::sales::quotation::QuotationQuery {
            customer_id: None,
            status: req.status.and_then(|s| {
                abt_core::sales::quotation::QuotationStatus::from_i16(s as i16)
            }),
            date_from: None,
            date_to: None,
            keyword: req.keyword,
        };
        let page = PageParams::new(
            req.pagination.as_ref().map(|p| p.page).unwrap_or(1),
            req.pagination.as_ref().map(|p| p.page_size).unwrap_or(20),
        );

        let result = srv.list(ctx, filter, page).await
            .map_err(domain_to_status)?;

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
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        match req.status() {
            QuotationStatus::Submitted => {
                srv.submit(ctx, req.quotation_id).await
                    .map_err(domain_to_status)?;
            }
            QuotationStatus::Accepted => {
                srv.accept(ctx, req.quotation_id).await
                    .map_err(domain_to_status)?;
            }
            QuotationStatus::Rejected => {
                srv.reject(ctx, req.quotation_id).await
                    .map_err(domain_to_status)?;
            }
            QuotationStatus::Expired => {
                srv.expire(ctx, req.quotation_id).await
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
