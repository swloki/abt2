//! Term gRPC Handler — 委托给 abt-core CategoryService

use abt_core::master_data::category::CategoryService;
use abt_core::shared::types::{PageParams, ServiceContext};

use crate::generated::abt::v1::{
    abt_term_service_server::AbtTermService as GrpcTermService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

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

fn category_to_term_response(c: &abt_core::master_data::category::Category) -> TermResponse {
    TermResponse {
        term_id: c.category_id,
        term_name: c.category_name.clone(),
        term_parent: c.parent_id,
        taxonomy: "category".to_string(),
        term_meta: Some(TermMeta { count: c.meta.count }),
    }
}

fn tree_to_term_tree(node: &abt_core::master_data::category::CategoryTree) -> TermTreeResponse {
    TermTreeResponse {
        term_id: node.category_id,
        term_name: node.category_name.clone(),
        term_parent: node.parent_id,
        taxonomy: "category".to_string(),
        term_meta: Some(TermMeta { count: node.meta.count }),
        children: node.children.iter().map(tree_to_term_tree).collect(),
    }
}

#[tonic::async_trait]
impl GrpcTermService for TermHandler {
    #[require_permission(Resource::Term, Action::Read)]
    async fn get_term_tree(
        &self,
        _request: Request<GetTermTreeRequest>,
    ) -> GrpcResult<TermTreeListResponse> {
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let tree = srv.get_tree(ctx, None, None)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(TermTreeListResponse {
            items: tree.iter().map(tree_to_term_tree).collect(),
        }))
    }

    #[require_permission(Resource::Term, Action::Read)]
    async fn list_terms(
        &self,
        _request: Request<ListTermsRequest>,
    ) -> GrpcResult<TermListResponse> {
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.list(
            ctx,
            abt_core::master_data::category::CategoryQuery::default(),
            PageParams::new(1, 1000),
        )
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(TermListResponse {
            items: result.items.iter().map(category_to_term_response).collect(),
        }))
    }

    #[require_permission(Resource::Term, Action::Read)]
    async fn get_term_children(
        &self,
        request: Request<GetTermChildrenRequest>,
    ) -> GrpcResult<TermListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.list(
            ctx,
            abt_core::master_data::category::CategoryQuery {
                parent_id: if req.parent_id == 0 { None } else { Some(req.parent_id) },
                name: None,
            },
            PageParams::new(1, 1000),
        )
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(TermListResponse {
            items: result.items.iter().map(category_to_term_response).collect(),
        }))
    }

    #[require_permission(Resource::Term, Action::Write)]
    async fn create_term(
        &self,
        request: Request<CreateTermRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req.clone()))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create(
            ctx,
            abt_core::master_data::category::CreateCategoryReq {
                category_name: req.term_name,
                parent_id: req.term_parent,
            },
        )
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Term, Action::Write)]
    async fn update_term(
        &self,
        request: Request<UpdateTermRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req.clone()))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(
            ctx,
            req.term_id,
            abt_core::master_data::category::UpdateCategoryReq {
                category_name: Some(req.term_name),
            },
        )
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Term, Action::Delete)]
    async fn delete_term(
        &self,
        request: Request<DeleteTermRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete(ctx, req.term_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
