//! SalesOrder gRPC Handler — 委托给 abt-core SalesOrderService

use abt_core::sales::sales_order::SalesOrderService;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    sales_order_service_server::SalesOrderService as GrpcSalesOrderService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

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

fn sales_order_status_to_proto(status: abt_core::sales::sales_order::SalesOrderStatus) -> SalesOrderStatus {
    match status {
        abt_core::sales::sales_order::SalesOrderStatus::Draft => SalesOrderStatus::Draft,
        abt_core::sales::sales_order::SalesOrderStatus::Confirmed => SalesOrderStatus::Confirmed,
        abt_core::sales::sales_order::SalesOrderStatus::InProduction => SalesOrderStatus::InProgress,
        abt_core::sales::sales_order::SalesOrderStatus::PartiallyShipped => SalesOrderStatus::PartiallyShipped,
        abt_core::sales::sales_order::SalesOrderStatus::Shipped => SalesOrderStatus::Shipped,
        abt_core::sales::sales_order::SalesOrderStatus::Completed => SalesOrderStatus::Completed,
        abt_core::sales::sales_order::SalesOrderStatus::Cancelled => SalesOrderStatus::Cancelled,
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

fn order_to_proto(o: &abt_core::sales::sales_order::SalesOrder) -> SalesOrder {
    SalesOrder {
        order_id: o.id,
        order_no: o.doc_number.clone(),
        quotation_id: 0,
        customer_name: String::new(),
        contact_person: String::new(),
        contact_phone: String::new(),
        status: sales_order_status_to_proto(o.status) as i32,
        total_amount: decimal_to_string(o.total_amount),
        remark: o.remark.clone(),
        delivery_date: o.order_date
            .and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .and_utc().timestamp(),
        created_at: o.created_at.timestamp(),
        updated_at: o.updated_at.timestamp(),
        operator_id: o.operator_id,
        items: vec![],
        customer_id: o.customer_id,
        contact_id: o.contact_id,
        sales_rep_id: o.sales_rep_id,
        total_cost: decimal_to_string(o.total_cost),
        payment_terms: o.payment_terms.clone(),
        delivery_terms: o.delivery_terms.clone(),
        delivery_address: o.delivery_address.clone(),
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
        let srv = state.sales_order_core_service();

        // 如果有 quotation_id，使用 create_from_quotation
        if req.quotation_id > 0 {
            let mut tx = state.begin_core_transaction().await
                .map_err(error::err_to_status)?;
            let ctx = ServiceContext::new(&mut tx, auth.user_id);
            let id = srv.create_from_quotation(ctx, req.quotation_id).await
                .map_err(domain_to_status)?;
            tx.commit().await.map_err(error::sqlx_err_to_status)?;
            return Ok(Response::new(U64Response { value: id as u64 }));
        }

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let items: Vec<abt_core::sales::sales_order::CreateSalesOrderItemReq> = req.items.iter().map(|i| {
            abt_core::sales::sales_order::CreateSalesOrderItemReq {
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

        let create_req = abt_core::sales::sales_order::CreateSalesOrderReq {
            customer_id: req.customer_id,
            contact_id: req.contact_id,
            items,
            payment_terms: if req.payment_terms.is_empty() { None } else { Some(req.payment_terms) },
            delivery_terms: if req.delivery_terms.is_empty() { None } else { Some(req.delivery_terms) },
            delivery_address: if req.delivery_address.is_empty() { None } else { Some(req.delivery_address) },
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        let id = srv.create(ctx, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn create_sales_order_from_quotation(
        &self,
        request: Request<CreateSalesOrderFromQuotationRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create_from_quotation(ctx, req.quotation_id).await
            .map_err(domain_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_order(
        &self,
        request: Request<UpdateSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let update_req = abt_core::sales::sales_order::UpdateSalesOrderReq {
            customer_id: if req.customer_id > 0 { Some(req.customer_id) } else { None },
            contact_id: if req.contact_id > 0 { Some(req.contact_id) } else { None },
            payment_terms: if req.payment_terms.is_empty() { None } else { Some(req.payment_terms) },
            delivery_terms: if req.delivery_terms.is_empty() { None } else { Some(req.delivery_terms) },
            delivery_address: if req.delivery_address.is_empty() { None } else { Some(req.delivery_address) },
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        srv.update_header(ctx, req.order_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_sales_order(
        &self,
        request: Request<DeleteSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let o = srv.find_by_id(ctx, req.order_id).await
            .map_err(domain_to_status)?;

        if o.status != abt_core::sales::sales_order::SalesOrderStatus::Draft {
            return Err(tonic::Status::failed_precondition("仅草稿状态的销售订单可以删除"));
        }

        let now = chrono::Utc::now().naive_utc();
        sqlx::query("UPDATE sales_orders SET deleted_at = $1, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(req.order_id)
            .execute(&mut *tx)
            .await
            .map_err(error::sqlx_err_to_status)?;

        sqlx::query("DELETE FROM sales_order_items WHERE order_id = $1")
            .bind(req.order_id)
            .execute(&mut *tx)
            .await
            .map_err(error::sqlx_err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_sales_order(
        &self,
        request: Request<GetSalesOrderRequest>,
    ) -> GrpcResult<SalesOrderResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();
        let customer_srv = state.customer_core_service();
        let product_srv = state.product_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let o = srv.find_by_id(ctx, req.order_id).await
            .map_err(domain_to_status)?;

        let mut proto = order_to_proto(&o);

        // Fetch customer info
        if o.customer_id > 0 {
            let ctx2 = ServiceContext::new(&mut tx, 0);
            if let Ok(customer) = customer_srv.get(ctx2, o.customer_id).await {
                proto.customer_name = customer.name.clone();
                if let Some(short) = &customer.short_name {
                    if proto.customer_name.is_empty() {
                        proto.customer_name = short.clone();
                    }
                }
                let ctx3 = ServiceContext::new(&mut tx, 0);
                if let Ok(contacts) = customer_srv.list_contacts(ctx3, o.customer_id).await {
                    if let Some(contact) = contacts.iter().find(|c| c.id == o.contact_id)
                        .or_else(|| contacts.first())
                    {
                        proto.contact_person = contact.name.clone();
                        proto.contact_phone = contact.phone.clone().unwrap_or_default();
                    }
                }
            }
        }

        // Fetch items
        let item_repo = abt_core::sales::sales_order::repo::SalesOrderItemRepo;
        let ctx4 = ServiceContext::new(&mut tx, 0);
        if let Ok(items) = item_repo.find_by_order_id(ctx4.executor, o.id).await {
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
                SalesOrderItem {
                    item_id: item.id,
                    order_id: item.order_id,
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
                    shipped_qty: decimal_to_string(item.shipped_qty),
                    returned_qty: decimal_to_string(item.returned_qty),
                    remark: String::new(),
                    line_no: item.line_no,
                    description: item.description.clone(),
                    unit_cost: decimal_to_string(item.unit_cost),
                    delivery_date: item.delivery_date
                        .map(|d| d.and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                        .and_utc().timestamp())
                        .unwrap_or(0),
                }
            }).collect();
        }

        Ok(Response::new(SalesOrderResponse {
            order: Some(proto),
        }))
    }

    async fn list_sales_orders(
        &self,
        request: Request<ListSalesOrdersRequest>,
    ) -> GrpcResult<SalesOrderListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();
        let customer_srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);

        let filter = abt_core::sales::sales_order::SalesOrderQuery {
            customer_id: None,
            status: req.status.and_then(|s| {
                abt_core::sales::sales_order::SalesOrderStatus::from_i16(s as i16)
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

        // Batch fetch customer names
        let customer_ids: Vec<i64> = result.items.iter()
            .map(|o| o.customer_id)
            .filter(|id| *id > 0)
            .collect();

        let mut customer_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
        for cid in customer_ids {
            if let std::collections::hash_map::Entry::Vacant(e) = customer_names.entry(cid) {
                let ctx2 = ServiceContext::new(&mut tx, 0);
                if let Ok(c) = customer_srv.get(ctx2, cid).await {
                    e.insert(c.name);
                }
            }
        }

        let proto_items: Vec<SalesOrder> = result.items.iter().map(|o| {
            let mut proto = order_to_proto(o);
            if let Some(name) = customer_names.get(&o.customer_id) {
                proto.customer_name = name.clone();
            }
            proto
        }).collect();

        Ok(Response::new(SalesOrderListResponse {
            items: proto_items,
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
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        match req.status() {
            SalesOrderStatus::Confirmed => {
                srv.confirm(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            SalesOrderStatus::InProgress => {
                srv.start_progress(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            SalesOrderStatus::Completed => {
                srv.complete(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            SalesOrderStatus::Cancelled => {
                srv.cancel(ctx, req.order_id).await
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
