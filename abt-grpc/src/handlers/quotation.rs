//! Quotation gRPC Handler — 委托给 abt-core QuotationService

use abt_core::sales::quotation::QuotationService;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
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
        abt_core::sales::quotation::QuotationStatus::Sent => QuotationStatus::Sent,
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
        unit_cost: String::new(),
        line_no: 0,
        description: String::new(),
        delivery_date: 0,
    }
}

fn quotation_to_proto(q: &abt_core::sales::quotation::Quotation) -> Quotation {
    Quotation {
        quotation_id: q.id,
        quotation_no: q.doc_number.clone(),
        customer_name: String::new(),
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
        items: vec![],
        customer_id: 0,
        contact_id: 0,
        sales_rep_id: 0,
        quotation_date: 0,
        total_cost: String::new(),
        estimated_margin: String::new(),
        payment_terms: String::new(),
        delivery_terms: String::new(),
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
                description: if i.description.is_empty() { None } else { Some(i.description.clone()) },
                quantity: parse_decimal(&i.quantity),
                unit: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
                unit_price: parse_decimal(&i.unit_price),
                unit_cost: {
                    let c = parse_decimal(&i.unit_cost);
                    if c == rust_decimal::Decimal::ZERO { None } else { Some(c) }
                },
                discount_rate: {
                    let d = parse_decimal(&i.discount);
                    if d == rust_decimal::Decimal::ZERO { None } else { Some(d) }
                },
                delivery_date: if i.delivery_date > 0 {
                    chrono::DateTime::from_timestamp(i.delivery_date, 0)
                        .map(|dt| dt.date_naive())
                } else {
                    None
                },
            }
        }).collect();

        let create_req = abt_core::sales::quotation::CreateQuotationReq {
            customer_id: req.customer_id,
            contact_id: req.contact_id,
            valid_until,
            items,
            payment_terms: if req.payment_terms.is_empty() { None } else { Some(req.payment_terms) },
            delivery_terms: if req.delivery_terms.is_empty() { None } else { Some(req.delivery_terms) },
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
                description: if i.description.is_empty() { None } else { Some(i.description.clone()) },
                quantity: parse_decimal(&i.quantity),
                unit: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
                unit_price: parse_decimal(&i.unit_price),
                unit_cost: {
                    let c = parse_decimal(&i.unit_cost);
                    if c == rust_decimal::Decimal::ZERO { None } else { Some(c) }
                },
                discount_rate: {
                    let d = parse_decimal(&i.discount);
                    if d == rust_decimal::Decimal::ZERO { None } else { Some(d) }
                },
                delivery_date: if i.delivery_date > 0 {
                    chrono::DateTime::from_timestamp(i.delivery_date, 0)
                        .map(|dt| dt.date_naive())
                } else {
                    None
                },
            }
        }).collect();

        let update_req = abt_core::sales::quotation::UpdateQuotationReq {
            customer_id: if req.customer_id > 0 { Some(req.customer_id) } else { None },
            contact_id: if req.contact_id > 0 { Some(req.contact_id) } else { None },
            sales_rep_id: None,
            valid_until,
            payment_terms: if req.payment_terms.is_empty() { None } else { Some(req.payment_terms) },
            delivery_terms: if req.delivery_terms.is_empty() { None } else { Some(req.delivery_terms) },
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
        let q = srv.find_by_id(ctx, req.quotation_id).await
            .map_err(domain_to_status)?;

        // Only draft quotations can be deleted
        if q.status != abt_core::sales::quotation::QuotationStatus::Draft {
            return Err(tonic::Status::failed_precondition("仅草稿状态的报价单可以删除"));
        }

        // Soft delete: set deleted_at
        let now = chrono::Utc::now().naive_utc();
        sqlx::query("UPDATE quotations SET deleted_at = $1, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(req.quotation_id)
            .execute(&mut *tx)
            .await
            .map_err(error::sqlx_err_to_status)?;

        // Also remove items
        let item_repo = abt_core::sales::quotation::repo::QuotationItemRepo;
        item_repo.delete_by_quotation_id(&mut *tx, req.quotation_id).await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_quotation(
        &self,
        request: Request<GetQuotationRequest>,
    ) -> GrpcResult<QuotationResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_core_service();
        let customer_srv = state.customer_core_service();
        let product_srv = state.product_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let q = srv.find_by_id(ctx, req.quotation_id).await
            .map_err(domain_to_status)?;

        let mut proto = quotation_to_proto(&q);
        proto.customer_id = q.customer_id;
        proto.contact_id = q.contact_id;
        proto.sales_rep_id = q.sales_rep_id;
        proto.quotation_date = q.quotation_date
            .and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .and_utc().timestamp();
        proto.total_cost = decimal_to_string(q.total_cost);
        proto.estimated_margin = decimal_to_string(q.estimated_margin);
        proto.payment_terms = q.payment_terms.clone();
        proto.delivery_terms = q.delivery_terms.clone();

        // Fetch customer info
        if q.customer_id > 0 {
            let ctx2 = ServiceContext::new(&mut tx, 0);
            if let Ok(customer) = customer_srv.get(ctx2, q.customer_id).await {
                proto.customer_name = customer.name.clone();
                if let Some(short) = &customer.short_name {
                    if proto.customer_name.is_empty() {
                        proto.customer_name = short.clone();
                    }
                }
                // Fetch contacts for contact info
                let ctx3 = ServiceContext::new(&mut tx, 0);
                if let Ok(contacts) = customer_srv.list_contacts(ctx3, q.customer_id).await {
                    if let Some(contact) = contacts.iter().find(|c| c.id == q.contact_id)
                        .or_else(|| contacts.first())
                    {
                        proto.contact_person = contact.name.clone();
                        proto.contact_phone = contact.phone.clone().unwrap_or_default();
                    }
                }
            }
        }

        // Fetch items
        let ctx4 = ServiceContext::new(&mut tx, 0);
        if let Ok(items) = srv.list_items(ctx4, q.id).await {
            // Batch fetch product names
            let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).filter(|id| *id > 0).collect();
            let mut product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
            let mut product_codes: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
            if !product_ids.is_empty() {
                let ctx5 = ServiceContext::new(&mut tx, 0);
                if let Ok(products) = product_srv.get_by_ids(ctx5, product_ids).await {
                    for p in &products {
                        product_names.insert(p.product_id, p.pdt_name.clone());
                        if !p.product_code.is_empty() {
                            product_codes.insert(p.product_id, p.product_code.clone());
                        }
                    }
                }
            }

            proto.items = items.iter().map(|item| {
                QuotationItem {
                    item_id: item.id,
                    quotation_id: item.quotation_id,
                    product_id: item.product_id,
                    product_code: product_codes.get(&item.product_id).cloned().unwrap_or_default(),
                    product_name: product_names.get(&item.product_id).cloned()
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| item.description.clone()),
                    unit: item.unit.clone(),
                    unit_price: decimal_to_string(item.unit_price),
                    quantity: decimal_to_string(item.quantity),
                    discount: decimal_to_string(item.discount_rate),
                    subtotal: decimal_to_string(item.amount),
                    remark: String::new(),
                    unit_cost: decimal_to_string(item.unit_cost),
                    line_no: item.line_no,
                    description: item.description.clone(),
                    delivery_date: item.delivery_date
                        .map(|d| d.and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                        .and_utc().timestamp())
                        .unwrap_or(0),
                }
            }).collect();
        }

        Ok(Response::new(QuotationResponse {
            quotation: Some(proto),
        }))
    }

    async fn list_quotations(
        &self,
        request: Request<ListQuotationsRequest>,
    ) -> GrpcResult<QuotationListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.quotation_core_service();
        let customer_srv = state.customer_core_service();

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

        // Collect customer IDs and batch-fetch customer names
        let customer_ids: Vec<i64> = result.items.iter()
            .map(|q| q.customer_id)
            .filter(|id| *id > 0)
            .collect();

        let mut customer_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
        for cid in customer_ids {
            if !customer_names.contains_key(&cid) {
                let ctx2 = ServiceContext::new(&mut tx, 0);
                if let Ok(c) = customer_srv.get(ctx2, cid).await {
                    customer_names.insert(cid, c.name);
                }
            }
        }

        let proto_items: Vec<Quotation> = result.items.iter().map(|q| {
            let mut proto = quotation_to_proto(q);
            proto.customer_id = q.customer_id;
            proto.contact_id = q.contact_id;
            proto.sales_rep_id = q.sales_rep_id;
            proto.quotation_date = q.quotation_date
                .and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                .and_utc().timestamp();
            proto.total_cost = decimal_to_string(q.total_cost);
            proto.estimated_margin = decimal_to_string(q.estimated_margin);
            proto.payment_terms = q.payment_terms.clone();
            proto.delivery_terms = q.delivery_terms.clone();
            if let Some(name) = customer_names.get(&q.customer_id) {
                proto.customer_name = name.clone();
            }
            proto
        }).collect();

        Ok(Response::new(QuotationListResponse {
            items: proto_items,
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
            QuotationStatus::Sent => {
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
