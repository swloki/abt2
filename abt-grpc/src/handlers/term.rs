//! Term gRPC Handler

use common::error;
use tonic::{Request, Response};
use crate::generated::abt::v1::{
    abt_term_service_server::AbtTermService as GrpcTermService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;

// Import trait to bring methods into scope
use abt::TermService;

pub struct TermHandler;

impl TermHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TermHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcTermService for TermHandler {
    #[require_permission("term", "read")]
    async fn get_term_tree(
        &self,
        request: Request<GetTermTreeRequest>,
    ) -> GrpcResult<TermTreeListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.term_service();

        let tree = srv.get_tree(&req.taxonomy).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(TermTreeListResponse {
            items: tree.into_iter().map(|t| t.into()).collect(),
        }))
    }

    #[require_permission("term", "read")]
    async fn list_terms(
        &self,
        request: Request<ListTermsRequest>,
    ) -> GrpcResult<TermListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.term_service();

        let terms = srv.list_by_taxonomy(&req.taxonomy).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(TermListResponse {
            items: terms.into_iter().map(|t| t.into()).collect(),
        }))
    }

    #[require_permission("term", "read")]
    async fn get_term_children(
        &self,
        request: Request<GetTermChildrenRequest>,
    ) -> GrpcResult<TermListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.term_service();

        let terms = srv.get_children(req.parent_id).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(TermListResponse {
            items: terms.into_iter().map(|t| t.into()).collect(),
        }))
    }

    #[require_permission("term", "write")]
    async fn create_term(
        &self,
        request: Request<CreateTermRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.term_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateTermRequest {
            term_name: req.term_name,
            term_parent: req.term_parent,
            taxonomy: req.taxonomy,
        };

        let id = srv.create(create_req, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission("term", "write")]
    async fn update_term(
        &self,
        request: Request<UpdateTermRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.term_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateTermRequest {
            term_name: req.term_name,
        };

        srv.update(req.term_id, update_req, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission("term", "delete")]
    async fn delete_term(
        &self,
        request: Request<DeleteTermRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.term_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(req.term_id, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
