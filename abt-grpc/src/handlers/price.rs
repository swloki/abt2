//! Price gRPC Handler — 委托给 abt-core ProductPriceService

use abt_core::master_data::price::ProductPriceService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use rust_decimal::Decimal;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_price_service_server::AbtPriceService as GrpcPriceService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

pub struct PriceHandler;

impl PriceHandler {
    pub fn new() -> Self { Self }
}

impl Default for PriceHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpcPriceService for PriceHandler {
    #[require_permission(Resource::Price, Action::Read)]
    async fn get_price_history(&self, request: Request<GetPriceHistoryRequest>) -> GrpcResult<PriceHistoryResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let query = abt_core::master_data::price::PriceQuery {
            product_id: Some(req.product_id),
            price_type: None,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(20));
        let result = srv.list_price_history(ctx, query, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(PriceHistoryResponse {
            items: result.items.into_iter().map(|entry| PriceLogEntryResponse {
                log_id: entry.log_id,
                product_id: entry.product_id,
                new_price: entry.new_price.to_string(),
                operator_id: Some(entry.operator_id.unwrap_or(0)),
                remark: Some(entry.remark),
                created_at: entry.created_at.timestamp(),
            }).collect(),
            total: result.total as u64,
            page: result.page,
            page_size: result.page_size,
        }))
    }

    #[require_permission(Resource::Price, Action::Write)]
    async fn update_price(&self, request: Request<UpdatePriceRequest>) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let new_price: Decimal = req.new_price.parse()
            .map_err(|_e| error::validation("new_price", "Invalid price format"))?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update_price(
            ctx,
            req.product_id,
            abt_core::master_data::price::PriceType::StandardCost,
            new_price,
            req.remark.unwrap_or_default(),
        ).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Price, Action::Read)]
    async fn list_all_price_history(&self, request: Request<ListAllPriceHistoryRequest>) -> GrpcResult<AllPriceHistoryResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.price_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let query = abt_core::master_data::price::PriceQuery {
            product_id: req.product_id,
            price_type: None,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(20));
        let result = srv.list_price_history(ctx, query, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(AllPriceHistoryResponse {
            items: result.items.into_iter().map(|entry| PriceLogWithProductResponse {
                log_id: entry.log_id,
                product_id: entry.product_id,
                product_name: String::new(),
                product_code: Some(String::new()),
                new_price: entry.new_price.to_string(),
                operator_id: Some(entry.operator_id.unwrap_or(0)),
                remark: Some(entry.remark),
                created_at: entry.created_at.timestamp(),
            }).collect(),
            total: result.total as u64,
            page: result.page,
            page_size: result.page_size,
        }))
    }
}
