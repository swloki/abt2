//! User gRPC Handler — 委托给 abt-core UserService

use abt_core::shared::identity::UserService;
use abt_core::shared::types::context::ServiceContext;
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{user_service_server::UserService as GrpcUserService, *};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

pub struct UserHandler;

impl UserHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UserHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcUserService for UserHandler {
    #[require_permission(Resource::User, Action::Write)]
    async fn create_user(&self, request: Request<CreateUserRequest>) -> GrpcResult<UserResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut *tx, auth.user_id);
        let display_name = if req.display_name.is_empty() {
            None
        } else {
            Some(req.display_name.as_str())
        };
        let created = srv.create_user(ctx, &req.username, &req.password, display_name, req.is_super_admin)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the created user with roles to return
        let mut conn = state.core_pool().acquire().await.map_err(error::sqlx_err_to_status)?;
        let ctx2 = ServiceContext::new(&mut *conn, auth.user_id);
        let user_with_roles = srv
            .get_user_with_roles(ctx2, created.user_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(user_with_roles.into()))
    }

    #[require_permission(Resource::User, Action::Write)]
    async fn update_user(&self, request: Request<UpdateUserRequest>) -> GrpcResult<UserResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut *tx, auth.user_id);

        // Update display_name if provided
        let display_name = if req.display_name.is_empty() {
            None
        } else {
            Some(req.display_name.as_str())
        };
        srv.update_user(ctx, req.user_id, display_name)
            .await
            .map_err(domain_to_status)?;

        // Update active status if changed
        let ctx2 = ServiceContext::new(&mut *tx, auth.user_id);
        srv.update_user_status(ctx2, req.user_id, req.is_active)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the updated user with roles to return
        let mut conn = state.core_pool().acquire().await.map_err(error::sqlx_err_to_status)?;
        let ctx3 = ServiceContext::new(&mut *conn, auth.user_id);
        let user_with_roles = srv
            .get_user_with_roles(ctx3, req.user_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(user_with_roles.into()))
    }

    #[require_permission(Resource::User, Action::Delete)]
    async fn delete_user(&self, request: Request<DeleteUserRequest>) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut *tx, auth.user_id);
        srv.delete_user(ctx, req.user_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::User, Action::Read)]
    async fn get_user(&self, request: Request<GetUserRequest>) -> GrpcResult<UserResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut conn = state.core_pool().acquire().await.map_err(error::sqlx_err_to_status)?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);
        let user_with_roles = srv
            .get_user_with_roles(ctx, req.user_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(user_with_roles.into()))
    }

    #[require_permission(Resource::User, Action::Read)]
    async fn list_users(&self, _request: Request<Empty>) -> GrpcResult<UserListResponse> {
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut conn = state.core_pool().acquire().await.map_err(error::sqlx_err_to_status)?;
        let ctx = ServiceContext::new(&mut *conn, 0);
        let users = srv
            .list_users_with_roles(ctx)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(UserListResponse {
            users: users.into_iter().map(|u| u.into()).collect(),
        }))
    }

    #[require_permission(Resource::User, Action::Read)]
    async fn get_users_by_ids(
        &self,
        request: Request<GetUsersByIdsRequest>,
    ) -> GrpcResult<UserListResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut conn = state.core_pool().acquire().await.map_err(error::sqlx_err_to_status)?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);
        let users = srv
            .get_users_by_ids(ctx, req.user_ids)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(UserListResponse {
            users: users.into_iter().map(|u| u.into()).collect(),
        }))
    }

    #[require_permission(Resource::User, Action::Write)]
    async fn assign_roles(&self, request: Request<AssignRolesRequest>) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut *tx, auth.user_id);
        srv.assign_roles(ctx, req.user_id, req.role_ids)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::User, Action::Write)]
    async fn remove_roles(&self, request: Request<RemoveRolesRequest>) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut *tx, auth.user_id);
        srv.remove_roles(ctx, req.user_id, req.role_ids)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::User, Action::Write)]
    async fn batch_assign_roles(
        &self,
        request: Request<BatchAssignRolesRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        // abt-core batch_assign_roles takes a single user_id; iterate over all user_ids
        for user_id in req.user_ids {
            let ctx = ServiceContext::new(&mut *tx, auth.user_id);
            srv.batch_assign_roles(ctx, user_id, req.role_ids.clone())
                .await
                .map_err(domain_to_status)?;
        }

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    async fn change_password(
        &self,
        request: Request<ChangePasswordRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut conn = state.core_pool().acquire().await.map_err(error::sqlx_err_to_status)?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);
        srv.change_password(ctx, auth.user_id, &req.old_password, &req.new_password)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(Empty {}))
    }
}
