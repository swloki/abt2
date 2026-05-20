use crate::generated::abt::v1::{
    reconciliation_service_server::ReconciliationService as GrpcReconciliationService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use common::error;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tonic::{Request, Response};

use abt::ReconciliationService;

pub struct ReconciliationHandler;

impl ReconciliationHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReconciliationHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn status_i16_to_proto(status: i16) -> i32 {
    match status {
        1 => ReconciliationStatus::Draft as i32,
        2 => ReconciliationStatus::Confirmed as i32,
        3 => ReconciliationStatus::Approved as i32,
        _ => ReconciliationStatus::Unspecified as i32,
    }
}

fn status_proto_to_i16(status: i32) -> i16 {
    match status {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    }
}

fn recon_item_to_proto(item: abt::ReconciliationItem) -> ReconciliationItem {
    ReconciliationItem {
        item_id: item.item_id,
        statement_id: item.statement_id,
        source_type: item.source_type,
        source_id: item.source_id.unwrap_or(0),
        product_id: item.product_id.unwrap_or(0),
        product_code: item.product_code.unwrap_or_default(),
        product_name: item.product_name.unwrap_or_default(),
        unit: item.unit.unwrap_or_default(),
        quantity: item.quantity.to_f64().unwrap_or(0.0),
        unit_price: item.unit_price.to_f64().unwrap_or(0.0),
        amount: item.amount.to_f64().unwrap_or(0.0),
        remark: item.remark.unwrap_or_default(),
    }
}

fn reconciliation_to_proto(s: abt::ReconciliationStatement) -> ReconciliationStatement {
    ReconciliationStatement {
        statement_id: s.statement_id,
        statement_no: s.statement_no,
        customer_name: s.customer_name,
        period_year: s.period_year as i32,
        period_month: s.period_month as i32,
        shipping_total: s.shipping_total.to_f64().unwrap_or(0.0),
        return_total: s.return_total.to_f64().unwrap_or(0.0),
        adjustment_total: s.adjustment_total.to_f64().unwrap_or(0.0),
        net_amount: s.net_amount.to_f64().unwrap_or(0.0),
        status: status_i16_to_proto(s.status),
        remark: s.remark.unwrap_or_default(),
        operator_id: s.operator_id.unwrap_or(0),
        created_at: s.created_at.and_utc().timestamp(),
        updated_at: s.updated_at.and_utc().timestamp(),
        items: s.items.into_iter().map(recon_item_to_proto).collect(),
    }
}

#[tonic::async_trait]
impl GrpcReconciliationService for ReconciliationHandler {
    async fn create_reconciliation(
        &self,
        request: Request<CreateReconciliationRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let stmt = abt::ReconciliationStatement {
            statement_id: 0,
            statement_no: String::new(),
            customer_name: req.customer_name,
            period_year: req.period_year as i16,
            period_month: req.period_month as i16,
            shipping_total: Decimal::ZERO,
            return_total: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            net_amount: Decimal::ZERO,
            status: 0,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items: vec![],
        };

        let id = srv.create(Some(auth.user_id), stmt, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn add_reconciliation_adjustment(
        &self,
        request: Request<AddReconciliationAdjustmentRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items: Vec<abt::ReconciliationItem> = req.items.iter().map(|item| abt::ReconciliationItem {
            item_id: 0,
            statement_id: req.statement_id,
            source_type: String::new(),
            source_id: None,
            product_id: if item.product_id > 0 { Some(item.product_id) } else { None },
            product_code: None,
            product_name: None,
            unit: None,
            quantity: Decimal::from_f64_retain(item.quantity).unwrap_or(Decimal::ZERO),
            unit_price: Decimal::from_f64_retain(item.unit_price).unwrap_or(Decimal::ZERO),
            amount: Decimal::from_f64_retain(item.amount).unwrap_or(Decimal::ZERO),
            remark: if item.remark.is_empty() { None } else { Some(item.remark.clone()) },
            created_at: chrono::NaiveDateTime::default(),
        }).collect();

        srv.add_adjustments(req.statement_id, items, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn update_reconciliation(
        &self,
        request: Request<UpdateReconciliationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let stmt = abt::ReconciliationStatement {
            statement_id: req.statement_id,
            statement_no: String::new(),
            customer_name: String::new(),
            period_year: 0,
            period_month: 0,
            shipping_total: Decimal::ZERO,
            return_total: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            net_amount: Decimal::ZERO,
            status: 0,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items: vec![],
        };

        srv.update(Some(auth.user_id), stmt, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_reconciliation(
        &self,
        request: Request<DeleteReconciliationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        srv.delete(req.statement_id, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_reconciliation(
        &self,
        request: Request<GetReconciliationRequest>,
    ) -> GrpcResult<ReconciliationResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let stmt = srv.get_by_id(req.statement_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("ReconciliationStatement", &req.statement_id.to_string()))?;

        Ok(Response::new(ReconciliationResponse {
            statement: Some(reconciliation_to_proto(stmt)),
        }))
    }

    async fn list_reconciliations(
        &self,
        request: Request<ListReconciliationsRequest>,
    ) -> GrpcResult<ReconciliationListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::ReconciliationQuery {
            keyword: req.keyword,
            status: req.status.map(status_proto_to_i16),
            period_year: req.period_year.map(|y| y as i16),
            period_month: req.period_month.map(|m| m as i16),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(ReconciliationListResponse {
            items: result.items.into_iter().map(reconciliation_to_proto).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    async fn update_reconciliation_status(
        &self,
        request: Request<UpdateReconciliationStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.reconciliation_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        let status = status_proto_to_i16(req.status);
        srv.update_status(req.statement_id, status, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
