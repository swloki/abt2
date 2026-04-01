//! Price gRPC Handler

use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{
    abt_price_service_server::AbtPriceService as GrpcPriceService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

use abt::{AllPriceHistoryQuery, PriceHistoryQuery, ProductPriceService};
use rust_decimal::Decimal;

pub struct PriceHandler;

impl PriceHandler {
    pub fn new() -> Self { Self }
}

impl Default for PriceHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpcPriceService for PriceHandler {
    async fn get_price_history(&self, request: Request<GetPriceHistoryRequest>) -> GrpcResult<PriceHistoryResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("price", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();

        let query = PriceHistoryQuery {
            product_id: req.product_id,
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
        };

        let result = srv.get_price_history(query, &state.pool()).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(PriceHistoryResponse {
            items: result.items.into_iter().map(|entry| PriceLogEntryResponse {
                log_id: entry.log_id,
                product_id: entry.product_id,
                old_price: entry.old_price.map(|p| p.to_string()).unwrap_or_default(),
                new_price: entry.new_price.to_string(),
                operator_id: entry.operator_id,
                remark: entry.remark,
                created_at: entry.created_at.timestamp(),
            }).collect(),
            total: result.total as u64,
            page: result.page as u32,
            page_size: result.page_size as u32,
        }))
    }

    async fn update_price(&self, request: Request<UpdatePriceRequest>) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("price", "write").map_err(|e| Status::permission_denied(e.to_string()))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();
        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let new_price: Decimal = req.new_price.parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid price: {}", e)))?;

        srv.update_price(
            req.product_id,
            new_price,
            req.operator_id,
            req.remark.as_deref(),
            &mut tx,
        ).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn list_all_price_history(&self, request: Request<ListAllPriceHistoryRequest>) -> GrpcResult<AllPriceHistoryResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("price", "read").map_err(|e| Status::permission_denied(e.to_string()))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();

        let query = AllPriceHistoryQuery {
            product_id: req.product_id,
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
            product_name: req.product_name,
            product_code: req.product_code,
        };

        let result = srv.list_all_price_history(query, &state.pool()).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(AllPriceHistoryResponse {
            items: result.items.into_iter().map(|entry| PriceLogWithProductResponse {
                log_id: entry.log_id,
                product_id: entry.product_id,
                product_name: entry.product_name,
                product_code: entry.product_code,
                old_price: entry.old_price.map(|p| p.to_string()).unwrap_or_default(),
                new_price: entry.new_price.to_string(),
                operator_id: entry.operator_id,
                remark: entry.remark,
                created_at: entry.created_at.timestamp(),
            }).collect(),
            total: result.total as u64,
            page: result.page as u32,
            page_size: result.page_size as u32,
        }))
    }
}
