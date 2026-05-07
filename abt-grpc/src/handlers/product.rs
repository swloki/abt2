//! Product gRPC Handler

use common::error;
use tonic::{Request, Response};
use abt_macros::require_permission;
use crate::permissions::PermissionCode;
use crate::generated::abt::v1::{
    abt_product_service_server::AbtProductService as GrpcProductService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

// Import trait to bring methods into scope
use abt::ProductService;
use abt::ProductWatcherService;

pub struct ProductHandler;

impl ProductHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProductHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcProductService for ProductHandler {
    #[require_permission(Resource::Product, Action::Read)]
    async fn list_products(
        &self,
        request: Request<ListProductsRequest>,
    ) -> GrpcResult<ProductListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let query = abt::ProductQuery {
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
            pdt_name: req.keyword,
            term_id: req.term_id,
            product_code: req.product_code,
        };

        let (items, total) = srv.query(query).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(ProductListResponse {
            items: items.into_iter().map(|p| p.into()).collect(),
            total: total as u64,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(12),
        }))
    }

    #[require_permission(Resource::Product, Action::Read)]
    async fn get_product(
        &self,
        request: Request<GetProductRequest>,
    ) -> GrpcResult<ProductResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let product = srv.find(req.product_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Product", &req.product_id.to_string()))?;

        Ok(Response::new(product.into()))
    }

    #[require_permission(Resource::Product, Action::Read)]
    async fn get_products_by_ids(
        &self,
        request: Request<GetProductsByIdsRequest>,
    ) -> GrpcResult<ProductsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let products = srv.find_by_ids(&req.product_ids).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(ProductsResponse {
            items: products.into_iter().map(|p| p.into()).collect(),
        }))
    }

    #[require_permission(Resource::Product, Action::Write)]
    async fn create_product(
        &self,
        request: Request<CreateProductRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let product = abt::Product {
            product_id: 0,
            pdt_name: req.pdt_name,
            product_code: req.product_code,
            unit: req.unit,
            meta: req.meta.map(|m| m.into()).unwrap_or_default(),
        };

        let id = srv.create(product, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Product, Action::Write)]
    async fn update_product(
        &self,
        request: Request<UpdateProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let product = abt::Product {
            product_id: req.product_id,
            pdt_name: req.pdt_name,
            product_code: req.product_code,
            unit: req.unit,
            meta: req.meta.map(|m| m.into()).unwrap_or_default(),
        };

        srv.update(req.product_id, product, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Product, Action::Delete)]
    async fn delete_product(
        &self,
        request: Request<DeleteProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        // 先检查产品是否被 BOM 使用
        let (is_used, boms, _total) = srv.check_product_usage(req.product_id, Some(1), Some(10)).await
            .map_err(error::err_to_status)?;

        if is_used {
            let bom_names: Vec<String> = boms.iter().map(|b| b.bom_name.clone()).collect();
            return Err(tonic::Status::failed_precondition(
                format!("产品正在以下 BOM 中使用，无法删除: {}", bom_names.join(", "))
            ));
        }

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(req.product_id, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Product, Action::Read)]
    async fn check_product_usage(
        &self,
        request: Request<CheckProductUsageRequest>,
    ) -> GrpcResult<CheckProductUsageResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let (is_used, boms, total) = srv.check_product_usage(req.product_id, req.page, req.page_size).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(CheckProductUsageResponse {
            is_used,
            used_in_boms: boms.into_iter().map(|b| BomReference {
                bom_id: b.bom_id,
                bom_name: b.bom_name,
            }).collect(),
            total_boms: total,
        }))
    }

    async fn watch_product(
        &self,
        request: Request<WatchProductRequest>,
    ) -> GrpcResult<WatchProductResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_watcher_service();

        let override_val = req
            .safety_stock_override
            .as_deref()
            .and_then(|s| s.parse::<rust_decimal::Decimal>().ok());

        if let Some(val) = override_val {
            if val <= rust_decimal::Decimal::ZERO {
                return Err(error::validation(
                    "safety_stock_override",
                    "安全库存覆盖值必须大于 0",
                ));
            }
        }

        let is_new = srv
            .watch_product(auth.user_id, req.product_id, override_val)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(WatchProductResponse { is_new }))
    }

    async fn unwatch_product(
        &self,
        request: Request<UnwatchProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_watcher_service();

        let found = srv
            .unwatch_product(auth.user_id, req.product_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: found }))
    }

    async fn list_watched_products(
        &self,
        request: Request<ListWatchedProductsRequest>,
    ) -> GrpcResult<ListWatchedProductsResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_watcher_service();

        let page = req.page.unwrap_or(1);
        let page_size = req.page_size.unwrap_or(20);

        let (items, total) = srv
            .list_watched_products(auth.user_id, page, page_size)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(ListWatchedProductsResponse {
            items: items
                .into_iter()
                .map(|p| WatchedProduct {
                    product_id: p.product_id,
                    product_code: p.product_code,
                    product_name: p.product_name,
                    current_quantity: p.current_quantity.to_string(),
                    effective_safety_stock: p.effective_safety_stock.to_string(),
                    is_alerting: p.is_alerting,
                })
                .collect(),
            total: total as u64,
            page,
            page_size,
        }))
    }
}
