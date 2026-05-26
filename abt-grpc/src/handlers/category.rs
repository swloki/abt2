use abt_core::master_data::category::{
    CategoryService, CreateCategoryReq, CategoryQuery, UpdateCategoryReq,
};
use abt_core::shared::types::{PageParams, ServiceContext};

use crate::generated::abt::v1::{
    abt_category_service_server::AbtCategoryService as GrpcCategoryService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

pub struct CategoryHandler;

impl CategoryHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CategoryHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn domain_to_status(e: abt_core::shared::types::DomainError) -> tonic::Status {
    use abt_core::shared::types::DomainError;
    match e {
        DomainError::NotFound(msg) => tonic::Status::not_found(msg),
        DomainError::Duplicate(msg) => tonic::Status::already_exists(msg),
        DomainError::PermissionDenied(msg) => tonic::Status::permission_denied(msg),
        DomainError::BusinessRule(msg) => tonic::Status::failed_precondition(msg),
        DomainError::Validation(msg) => tonic::Status::invalid_argument(msg),
        DomainError::ConcurrentConflict => tonic::Status::aborted("Concurrent conflict"),
        DomainError::InvalidStateTransition { from, to } => {
            tonic::Status::failed_precondition(format!("Invalid state transition: {from} -> {to}"))
        }
        DomainError::Internal(e) => tonic::Status::internal(e.to_string()),
    }
}

fn category_to_response(c: &abt_core::master_data::category::Category) -> CategoryResponse {
    CategoryResponse {
        category_id: c.category_id,
        category_name: c.category_name.clone(),
        parent_id: c.parent_id,
        path: c.path.clone(),
        product_count: c.meta.count,
        created_at: c.created_at.timestamp(),
        updated_at: c.updated_at.map(|t| t.timestamp()).unwrap_or(0),
    }
}

fn tree_to_proto(node: &abt_core::master_data::category::CategoryTree) -> CategoryTreeNode {
    CategoryTreeNode {
        category_id: node.category_id,
        category_name: node.category_name.clone(),
        parent_id: node.parent_id,
        path: node.path.clone(),
        product_count: node.meta.count,
        children: node.children.iter().map(tree_to_proto).collect(),
    }
}

#[tonic::async_trait]
impl GrpcCategoryService for CategoryHandler {
    #[require_permission(Resource::Bom, Action::Write)]
    async fn create_category(
        &self,
        request: Request<CreateCategoryRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req.clone()))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv
            .create(ctx, CreateCategoryReq {
                category_name: req.category_name,
                parent_id: req.parent_id,
            })
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn update_category(
        &self,
        request: Request<UpdateCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req.clone()))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(ctx, req.category_id, UpdateCategoryReq {
            category_name: Some(req.category_name),
        })
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn delete_category(
        &self,
        request: Request<DeleteCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let category_id = req.category_id;
        srv.delete(ctx, category_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_category(
        &self,
        request: Request<GetCategoryRequest>,
    ) -> GrpcResult<CategoryResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let category = srv
            .get(ctx, req.category_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(category_to_response(&category)))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn list_categories(
        &self,
        request: Request<ListCategoriesRequest>,
    ) -> GrpcResult<CategoryListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv
            .list(
                ctx,
                CategoryQuery {
                    name: req.name,
                    parent_id: req.parent_id,
                },
                PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(20)),
            )
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(CategoryListResponse {
            items: result.items.iter().map(category_to_response).collect(),
            total: result.total as u64,
        }))
    }

    #[require_permission(Resource::Bom, Action::Read)]
    async fn get_category_tree(
        &self,
        request: Request<GetCategoryTreeRequest>,
    ) -> GrpcResult<CategoryTreeResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let tree = srv
            .get_tree(ctx, req.root_id, req.depth_limit)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(CategoryTreeResponse {
            nodes: tree.iter().map(tree_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn move_category(
        &self,
        request: Request<MoveCategoryRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let (category_id, new_parent_id) = (req.category_id, req.new_parent_id);
        srv.move_to(ctx, category_id, new_parent_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn assign_products(
        &self,
        request: Request<AssignProductsRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req.clone()))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.assign_products(ctx, req.category_id, req.product_ids)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Bom, Action::Write)]
    async fn remove_products(
        &self,
        request: Request<RemoveProductsRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let auth = extract_auth(&Request::new(req.clone()))?;
        let state = AppState::get().await;
        let srv = state.category_service();

        let mut tx = state.begin_core_transaction().await.map_err(|e| {
            tonic::Status::internal(e.to_string())
        })?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.remove_products(ctx, req.category_id, req.product_ids)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
