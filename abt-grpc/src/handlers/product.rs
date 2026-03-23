//! Product gRPC Handler

use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{
    abt_product_service_server::AbtProductService as GrpcProductService,
    *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

// Import trait to bring methods into scope
use abt::ProductService;

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
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ProductListResponse {
            items: items.into_iter().map(|p| p.into()).collect(),
            total: total as u64,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(12),
        }))
    }

    async fn get_product(
        &self,
        request: Request<GetProductRequest>,
    ) -> GrpcResult<ProductResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let product = srv.find(req.product_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Product not found"))?;

        Ok(Response::new(product.into()))
    }

    async fn get_products_by_ids(
        &self,
        request: Request<GetProductsByIdsRequest>,
    ) -> GrpcResult<ProductsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let products = srv.find_by_ids(&req.product_ids).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ProductsResponse {
            items: products.into_iter().map(|p| p.into()).collect(),
        }))
    }

    async fn create_product(
        &self,
        request: Request<CreateProductRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let product = abt::Product {
            product_id: 0,
            pdt_name: req.pdt_name,
            meta: req.meta.map(|m| m.into()).unwrap_or_default(),
        };

        let id = srv.create(product, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_product(
        &self,
        request: Request<UpdateProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let product = abt::Product {
            product_id: req.product_id,
            pdt_name: req.pdt_name,
            meta: req.meta.map(|m| m.into()).unwrap_or_default(),
        };

        srv.update(req.product_id, product, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_product(
        &self,
        request: Request<DeleteProductRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        // 先检查产品是否被 BOM 使用
        let (is_used, boms) = srv.check_product_usage(req.product_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        if is_used {
            let bom_names: Vec<String> = boms.iter().map(|b| b.bom_name.clone()).collect();
            return Err(Status::failed_precondition(
                format!("产品正在以下 BOM 中使用，无法删除: {}", bom_names.join(", "))
            ));
        }

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.delete(req.product_id, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn check_product_usage(
        &self,
        request: Request<CheckProductUsageRequest>,
    ) -> GrpcResult<CheckProductUsageResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.product_service();

        let (is_used, boms) = srv.check_product_usage(req.product_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CheckProductUsageResponse {
            is_used,
            used_in_boms: boms.into_iter().map(|b| BomReference {
                bom_id: b.bom_id,
                bom_name: b.bom_name,
            }).collect(),
        }))
    }
}
