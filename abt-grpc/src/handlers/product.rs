//! Product gRPC Handler — 委托给 abt-core ProductService

use abt_core::master_data::product::ProductService;
use abt_core::shared::types::{PageParams, ServiceContext};
use common::error;
use tonic::{Request, Response};
use abt_macros::require_permission;
use crate::handlers::domain_to_status;
use crate::permissions::PermissionCode;
use crate::generated::abt::v1::{
    abt_product_service_server::AbtProductService as GrpcProductService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

// ProductWatcher 已迁移到 abt-core
use abt_core::master_data::product_watcher::ProductWatcherService;

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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let filter = abt_core::master_data::product::ProductQuery {
            name: req.keyword,
            code: req.product_code,
            status: None,
            owner_department_id: None,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(12));
        let result = srv.list(ctx, filter, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(ProductListResponse {
            items: result.items.into_iter().map(|p| p.into()).collect(),
            total: result.total as u64,
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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let product = srv.get(ctx, req.product_id).await
            .map_err(domain_to_status)?;

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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let products = srv.get_by_ids(ctx, req.product_ids).await
            .map_err(domain_to_status)?;

        Ok(Response::new(ProductsResponse {
            items: products.into_iter().map(|p| p.into()).collect(),
        }))
    }

    #[require_permission(Resource::Product, Action::Write)]
    async fn create_product(
        &self,
        request: Request<CreateProductRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let create_req = abt_core::master_data::product::CreateProductReq {
            name: req.pdt_name,
            unit: req.unit,
            status: abt_core::master_data::product::ProductStatus::Active,
            external_code: None,
            owner_department_id: None,
            meta: req.meta.map(|m| m.into()).unwrap_or(abt_core::master_data::product::ProductMeta {
                specification: String::new(),
                acquire_channel: String::new(),
                old_code: None,
            }),
        };

        let id = srv.create(ctx, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // H3Yun 同步由 ProductServiceImpl 发布的领域事件自动触发，无需手动调用

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Product, Action::Write)]
    async fn update_product(
        &self,
        request: Request<UpdateProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let update_req = abt_core::master_data::product::UpdateProductReq {
            name: Some(req.pdt_name),
            unit: Some(req.unit),
            external_code: None,
            owner_department_id: None,
            meta: req.meta.map(|m| m.into()),
        };

        srv.update(ctx, req.product_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // H3Yun 同步由 ProductServiceImpl 发布的领域事件自动触发，无需手动调用

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Product, Action::Delete)]
    async fn delete_product(
        &self,
        request: Request<DeleteProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        // 先检查产品是否被 BOM 使用
        let mut tx1 = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx1 = ServiceContext::new(&mut tx1, 0);
        let usage = srv.check_product_usage(ctx1, req.product_id, abt_core::master_data::product::UsageQuery {
            page: 1,
            page_size: 10,
        }).await.map_err(domain_to_status)?;
        drop(tx1);

        if !usage.items.is_empty() {
            let names: Vec<String> = usage.items.iter().map(|b| b.source_name.clone()).collect();
            return Err(tonic::Status::failed_precondition(
                format!("产品正在以下 BOM 中使用，无法删除: {}", names.join(", "))
            ));
        }

        let mut tx2 = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx2 = ServiceContext::new(&mut tx2, auth.user_id);
        srv.delete(ctx2, req.product_id).await
            .map_err(domain_to_status)?;
        tx2.commit().await.map_err(error::sqlx_err_to_status)?;

        // H3Yun 同步由 ProductServiceImpl 发布的领域事件自动触发，无需手动调用

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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.check_product_usage(ctx, req.product_id, abt_core::master_data::product::UsageQuery {
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(10),
        }).await.map_err(domain_to_status)?;

        Ok(Response::new(CheckProductUsageResponse {
            is_used: !result.items.is_empty(),
            used_in_boms: result.items.into_iter().map(|b| BomReference {
                bom_id: b.source_id,
                bom_name: b.source_name,
            }).collect(),
            total_boms: result.total as i64,
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

        let override_val = if let Some(ref s) = req.safety_stock_override {
            let val = s.parse::<rust_decimal::Decimal>().map_err(|_| {
                error::validation("safety_stock_override", "无效的小数值")
            })?;
            if val <= rust_decimal::Decimal::ZERO {
                return Err(error::validation(
                    "safety_stock_override",
                    "安全库存覆盖值必须大于 0",
                ));
            }
            Some(val)
        } else {
            None
        };

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let is_new = srv
            .watch_product(ctx, req.product_id, override_val)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let found = srv
            .unwatch_product(ctx, req.product_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let result = srv
            .list_watched_products(ctx, page, page_size)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(ListWatchedProductsResponse {
            items: result
                .items
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
            total: result.total as u64,
            page,
            page_size,
        }))
    }
}
