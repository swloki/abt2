//! Price gRPC Handler

use common::error;
use tonic::{Request, Response};
use crate::generated::abt::v1::{
    abt_price_service_server::AbtPriceService as GrpcPriceService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;

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
    #[require_permission("price", "read")]
    async fn get_price_history(&self, request: Request<GetPriceHistoryRequest>) -> GrpcResult<PriceHistoryResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();

        let query = PriceHistoryQuery {
            product_id: req.product_id,
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
        };

        let result = srv.get_price_history(query, &state.pool()).await
            .map_err(error::err_to_status)?;

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

    #[require_permission("price", "write")]
    async fn update_price(&self, request: Request<UpdatePriceRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();
        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let new_price: Decimal = req.new_price.parse()
            .map_err(|_e| error::validation("new_price", "Invalid price format"))?;

        srv.update_price(
            req.product_id,
            new_price,
            req.operator_id,
            req.remark.as_deref(),
            &mut tx,
        ).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission("price", "read")]
    async fn list_all_price_history(&self, request: Request<ListAllPriceHistoryRequest>) -> GrpcResult<AllPriceHistoryResponse> {
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
            .map_err(error::err_to_status)?;

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
