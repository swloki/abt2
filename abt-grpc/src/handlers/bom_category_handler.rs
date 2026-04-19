//! BomCategory gRPC Handler

use crate::generated::abt::v1::{
    abt_bom_category_service_server::AbtBomCategoryService as GrpcBomCategoryService, *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use abt::BomCategoryService;

pub struct BomCategoryHandler;

impl BomCategoryHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BomCategoryHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcBomCategoryService for BomCategoryHandler {
    #[require_permission(Resource::Bom, Action::Write)]
    async fn create_bom_category(
        &self,
        request: Request<CreateBomCategoryRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateBomCategoryRequest {
            bom_category_name: req.bom_category_name,
        };

        let bom_category_id = srv
            .create(create_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: bom_category_id as u64 }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn update_bom_category(
        &self,
        request: Request<UpdateBomCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateBomCategoryRequest {
            bom_category_name: req.bom_category_name,
        };

        srv.update(req.bom_category_id, update_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn delete_bom_category(
        &self,
        request: Request<DeleteBomCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete(req.bom_category_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_bom_category(
        &self,
        request: Request<GetBomCategoryRequest>,
    ) -> GrpcResult<BomCategoryResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let category = srv
            .get(req.bom_category_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("BomCategory", &req.bom_category_id.to_string()))?;

        Ok(Response::new(category.into()))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn list_bom_categories(
        &self,
        request: Request<ListBomCategoriesRequest>,
    ) -> GrpcResult<BomCategoryListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let query = abt::BomCategoryQuery {
            keyword: if req.keyword.is_empty() { None } else { req.keyword },
            page: req.page,
            page_size: req.page_size,
        };

        let (categories, total) = srv
            .list(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomCategoryListResponse {
            items: categories.into_iter().map(|c| c.into()).collect(),
            total: total as u64,
        }))
    }
}
