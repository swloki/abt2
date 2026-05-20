use common::error;
use rust_decimal::Decimal;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    purchase_settlement_service_server::PurchaseSettlementService as GrpcSettlementService,
    *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

use abt::{StatementService, InvoiceService, PaymentService};

fn timestamp_to_date(ts: i64) -> Result<chrono::NaiveDate, tonic::Status> {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.date_naive())
        .ok_or_else(|| error::validation("date", "Invalid timestamp"))
}

fn date_to_timestamp(d: &chrono::NaiveDate) -> i64 {
    d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()
}

pub struct SettlementHandler;

impl SettlementHandler {
    pub fn new() -> Self { Self }
}

impl Default for SettlementHandler {
    fn default() -> Self { Self::new() }
}

fn statement_status_to_proto(status: i16) -> i32 {
    match status {
        1 => StatementStatus::Pending as i32,
        2 => StatementStatus::Confirmed as i32,
        3 => StatementStatus::Disputed as i32,
        _ => StatementStatus::Unspecified as i32,
    }
}

fn proto_to_statement_status(status: i32) -> i16 {
    match StatementStatus::try_from(status).unwrap_or(StatementStatus::Unspecified) {
        StatementStatus::Pending => 1,
        StatementStatus::Confirmed => 2,
        StatementStatus::Disputed => 3,
        _ => 0,
    }
}

fn invoice_status_to_proto(status: i16) -> i32 {
    match status {
        1 => InvoiceStatus::Registered as i32,
        2 => InvoiceStatus::Verified as i32,
        _ => InvoiceStatus::Unspecified as i32,
    }
}

fn proto_to_invoice_status(status: i32) -> i16 {
    match InvoiceStatus::try_from(status).unwrap_or(InvoiceStatus::Unspecified) {
        InvoiceStatus::Registered => 1,
        InvoiceStatus::Verified => 2,
        _ => 0,
    }
}

fn payment_status_to_proto(status: i16) -> i32 {
    match status {
        1 => PaymentStatus::Pending as i32,
        2 => PaymentStatus::Approved as i32,
        3 => PaymentStatus::Paid as i32,
        _ => PaymentStatus::Unspecified as i32,
    }
}

fn proto_to_payment_status(status: i32) -> i16 {
    match PaymentStatus::try_from(status).unwrap_or(PaymentStatus::Unspecified) {
        PaymentStatus::Pending => 1,
        PaymentStatus::Approved => 2,
        PaymentStatus::Paid => 3,
        _ => 0,
    }
}

fn statement_with_items_to_proto(wi: &abt::models::StatementWithItems) -> PurchaseStatement {
    let s = &wi.statement;
    PurchaseStatement {
        statement_id: s.statement_id,
        statement_no: s.statement_no.clone(),
        supplier_id: s.supplier_id,
        supplier_name: String::new(),
        period_start: date_to_timestamp(&s.period_start),
        period_end: date_to_timestamp(&s.period_end),
        total_amount: s.total_amount.to_string(),
        status: statement_status_to_proto(s.status),
        remark: s.remark.clone().unwrap_or_default(),
        operator_id: s.operator_id.unwrap_or(0),
        created_at: s.created_at.timestamp(),
        updated_at: s.updated_at.timestamp(),
        items: wi.items.iter().map(|i| StatementItem {
            item_id: i.item_id,
            statement_id: i.statement_id,
            po_id: i.po_id,
            po_no: i.po_no.clone().unwrap_or_default(),
            product_id: i.product_id,
            product_name: i.product_name.clone().unwrap_or_default(),
            quantity: i.quantity.to_string(),
            unit_price: i.unit_price.to_string(),
            amount: i.amount.to_string(),
        }).collect(),
    }
}

fn statement_detail_to_proto(d: &abt::models::StatementDetail) -> PurchaseStatement {
    PurchaseStatement {
        statement_id: d.statement_id,
        statement_no: d.statement_no.clone(),
        supplier_id: d.supplier_id,
        supplier_name: d.supplier_name.clone().unwrap_or_default(),
        period_start: date_to_timestamp(&d.period_start),
        period_end: date_to_timestamp(&d.period_end),
        total_amount: d.total_amount.to_string(),
        status: statement_status_to_proto(d.status),
        remark: d.remark.clone().unwrap_or_default(),
        operator_id: d.operator_id.unwrap_or(0),
        created_at: d.created_at.timestamp(),
        updated_at: d.updated_at.timestamp(),
        items: vec![],
    }
}

fn invoice_detail_to_proto(d: &abt::models::InvoiceDetail) -> PurchaseInvoice {
    PurchaseInvoice {
        invoice_id: d.invoice_id,
        invoice_no: d.invoice_no.clone(),
        supplier_id: d.supplier_id,
        supplier_name: d.supplier_name.clone().unwrap_or_default(),
        statement_id: d.statement_id.unwrap_or(0),
        statement_no: d.statement_no.clone().unwrap_or_default(),
        invoice_amount: d.invoice_amount.to_string(),
        invoice_date: date_to_timestamp(&d.invoice_date),
        status: invoice_status_to_proto(d.status),
        remark: d.remark.clone().unwrap_or_default(),
        operator_id: d.operator_id.unwrap_or(0),
        created_at: d.created_at.timestamp(),
    }
}

fn payment_detail_to_proto(d: &abt::models::PaymentDetail) -> PurchasePayment {
    PurchasePayment {
        payment_id: d.payment_id,
        payment_no: d.payment_no.clone(),
        supplier_id: d.supplier_id,
        supplier_name: d.supplier_name.clone().unwrap_or_default(),
        invoice_id: d.invoice_id.unwrap_or(0),
        invoice_no: d.invoice_no.clone().unwrap_or_default(),
        payment_amount: d.payment_amount.to_string(),
        payment_method: d.payment_method.clone().unwrap_or_default(),
        status: payment_status_to_proto(d.status),
        remark: d.remark.clone().unwrap_or_default(),
        operator_id: d.operator_id.unwrap_or(0),
        created_at: d.created_at.timestamp(),
        updated_at: d.updated_at.timestamp(),
    }
}

fn payment_to_proto(p: &abt::models::PurchasePayment) -> PurchasePayment {
    PurchasePayment {
        payment_id: p.payment_id,
        payment_no: p.payment_no.clone(),
        supplier_id: p.supplier_id,
        supplier_name: String::new(),
        invoice_id: p.invoice_id.unwrap_or(0),
        invoice_no: String::new(),
        payment_amount: p.payment_amount.to_string(),
        payment_method: p.payment_method.clone().unwrap_or_default(),
        status: payment_status_to_proto(p.status),
        remark: p.remark.clone().unwrap_or_default(),
        operator_id: p.operator_id.unwrap_or(0),
        created_at: p.created_at.timestamp(),
        updated_at: p.updated_at.timestamp(),
    }
}

#[tonic::async_trait]
impl GrpcSettlementService for SettlementHandler {
    #[require_permission(Resource::PurchaseSettlement, Action::Write)]
    async fn generate_statement(&self, request: Request<GenerateStatementRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.statement_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let period_start = timestamp_to_date(req.period_start)?;
        let period_end = timestamp_to_date(req.period_end)?;

        let id = srv.generate(
            req.supplier_id,
            period_start,
            period_end,
            Some(auth.user_id),
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Read)]
    async fn get_statement(&self, request: Request<GetStatementRequest>) -> GrpcResult<StatementResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.statement_service();

        let wi = srv.get_by_id(req.statement_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Statement", &req.statement_id.to_string()))?;

        Ok(Response::new(StatementResponse {
            statement: Some(statement_with_items_to_proto(&wi)),
        }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Read)]
    async fn list_statements(&self, request: Request<ListStatementsRequest>) -> GrpcResult<StatementListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.statement_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let period_start = req.period_start.map(timestamp_to_date).transpose()?;
        let period_end = req.period_end.map(timestamp_to_date).transpose()?;

        let query = abt::models::StatementQuery {
            supplier_id: req.supplier_id,
            status: req.status.map(proto_to_statement_status),
            period_start,
            period_end,
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(StatementListResponse {
            items: result.items.into_iter().map(|d| statement_detail_to_proto(&d)).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Write)]
    async fn update_statement_status(&self, request: Request<UpdateStatementStatusRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.statement_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let status = proto_to_statement_status(req.status);
        srv.update_status(req.statement_id, status, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Write)]
    async fn create_invoice(&self, request: Request<CreateInvoiceRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.invoice_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let invoice_amount: Decimal = req.invoice_amount.parse()
            .map_err(|_| error::validation("invoice_amount", "Invalid decimal format"))?;

        let invoice_date = timestamp_to_date(req.invoice_date)?;

        let statement_id = if req.statement_id == 0 { None } else { Some(req.statement_id) };

        let id = srv.create(
            req.invoice_no,
            req.supplier_id,
            statement_id,
            invoice_amount,
            invoice_date,
            empty_to_none(req.remark),
            Some(auth.user_id),
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Read)]
    async fn list_invoices(&self, request: Request<ListInvoicesRequest>) -> GrpcResult<InvoiceListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.invoice_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::models::InvoiceQuery {
            supplier_id: req.supplier_id,
            statement_id: req.statement_id,
            status: req.status.map(proto_to_invoice_status),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(InvoiceListResponse {
            items: result.items.into_iter().map(|d| invoice_detail_to_proto(&d)).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Write)]
    async fn update_invoice_status(&self, request: Request<UpdateInvoiceStatusRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.invoice_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let status = proto_to_invoice_status(req.status);
        srv.update_status(req.invoice_id, status, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Write)]
    async fn create_payment(&self, request: Request<CreatePaymentRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.payment_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let payment_amount: Decimal = req.payment_amount.parse()
            .map_err(|_| error::validation("payment_amount", "Invalid decimal format"))?;

        let invoice_id = if req.invoice_id == 0 { None } else { Some(req.invoice_id) };

        let id = srv.create(
            req.supplier_id,
            invoice_id,
            payment_amount,
            empty_to_none(req.payment_method),
            empty_to_none(req.remark),
            Some(auth.user_id),
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Read)]
    async fn get_payment(&self, request: Request<GetPaymentRequest>) -> GrpcResult<PaymentResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.payment_service();

        let payment = srv.get_by_id(req.payment_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Payment", &req.payment_id.to_string()))?;

        Ok(Response::new(PaymentResponse {
            payment: Some(payment_to_proto(&payment)),
        }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Read)]
    async fn list_payments(&self, request: Request<ListPaymentsRequest>) -> GrpcResult<PaymentListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.payment_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::models::PaymentQuery {
            supplier_id: req.supplier_id,
            status: req.status.map(proto_to_payment_status),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(PaymentListResponse {
            items: result.items.into_iter().map(|d| payment_detail_to_proto(&d)).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    #[require_permission(Resource::PurchaseSettlement, Action::Write)]
    async fn update_payment_status(&self, request: Request<UpdatePaymentStatusRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.payment_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let status = proto_to_payment_status(req.status);
        srv.update_status(req.payment_id, status, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
