//! BomCategory gRPC Handler — 委托给 abt-core BomCategoryService

use abt_core::master_data::bom::service::BomCategoryService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_bom_category_service_server::AbtBomCategoryService as GrpcBomCategoryService, *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;

pub struct BomCategoryHandler;

impl BomCategoryHandler {
    pub fn new() -> Self { Self }
}

impl Default for BomCategoryHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpcBomCategoryService for BomCategoryHandler {
    #[require_permission(Resource::Bom, Action::Write)]
    async fn create_bom_category(
        &self,
        request: Request<CreateBomCategoryRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create(ctx, abt_core::master_data::bom::model::CreateBomCategoryReq {
            bom_category_name: req.bom_category_name,
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn update_bom_category(
        &self,
        request: Request<UpdateBomCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(ctx, req.bom_category_id, abt_core::master_data::bom::model::UpdateBomCategoryReq {
            bom_category_name: Some(req.bom_category_name),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn delete_bom_category(
        &self,
        request: Request<DeleteBomCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete(ctx, req.bom_category_id).await.map_err(domain_to_status)?;

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

        // BomCategoryService 暂无 get 方法，使用 list + 事后过滤
        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.list(
            ctx,
            abt_core::master_data::bom::model::BomCategoryQuery::default(),
            PageParams::new(1, 99999),
        ).await.map_err(domain_to_status)?;

        let category = result.items.into_iter()
            .find(|c| c.bom_category_id == req.bom_category_id)
            .ok_or_else(|| domain_to_status(abt_core::shared::types::DomainError::not_found("BomCategory")))?;

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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let query = abt_core::master_data::bom::model::BomCategoryQuery {
            name: req.keyword,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(50));
        let result = srv.list(ctx, query, page).await.map_err(domain_to_status)?;

        Ok(Response::new(BomCategoryListResponse {
            items: result.items.into_iter().map(|c| c.into()).collect(),
            total: result.total as u64,
        }))
    }
}
