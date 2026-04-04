//! User gRPC Handler

use crate::generated::abt::v1::{user_service_server::UserService as GrpcUserService, *};
use crate::handlers::GrpcResult;
use crate::server::AppState;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use abt::UserService;
use crate::interceptors::auth::extract_auth;

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
    #[require_permission("user", "write")]
    async fn create_user(&self, request: Request<CreateUserRequest>) -> GrpcResult<UserResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateUserRequest {
            username: req.username,
            password: req.password,
            display_name: if req.display_name.is_empty() {
                None
            } else {
                Some(req.display_name)
            },
            is_super_admin: req.is_super_admin,
        };

        let user_id = srv
            .create(Some(auth.user_id), create_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        // Fetch the created user to return
        let user_with_roles = srv
            .get(user_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("User", &user_id.to_string()))?;

        Ok(Response::new(user_with_roles.into()))
    }

    #[require_permission("user", "write")]
    async fn update_user(&self, request: Request<UpdateUserRequest>) -> GrpcResult<UserResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateUserRequest {
            display_name: if req.display_name.is_empty() {
                None
            } else {
                Some(req.display_name)
            },
            is_active: Some(req.is_active),
        };

        srv.update(Some(auth.user_id), req.user_id, update_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        // Fetch the updated user to return
        let user_with_roles = srv
            .get(req.user_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("User", &req.user_id.to_string()))?;

        Ok(Response::new(user_with_roles.into()))
    }

    #[require_permission("user", "delete")]
    async fn delete_user(&self, request: Request<DeleteUserRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete(Some(auth.user_id), req.user_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission("user", "read")]
    async fn get_user(&self, request: Request<GetUserRequest>) -> GrpcResult<UserResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let user_with_roles = srv
            .get(req.user_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("User", &req.user_id.to_string()))?;

        Ok(Response::new(user_with_roles.into()))
    }

    #[require_permission("user", "read")]
    async fn list_users(&self, request: Request<Empty>) -> GrpcResult<UserListResponse> {
        let state = AppState::get().await;
        let srv = state.user_service();

        let users = srv
            .list()
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(UserListResponse {
            users: users.into_iter().map(|u| u.into()).collect(),
        }))
    }

    #[require_permission("user", "write")]
    async fn assign_roles(&self, request: Request<AssignRolesRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.assign_roles(Some(auth.user_id), req.user_id, req.role_ids, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission("user", "write")]
    async fn remove_roles(&self, request: Request<RemoveRolesRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.remove_roles(Some(auth.user_id), req.user_id, req.role_ids, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission("user", "write")]
    async fn batch_assign_roles(
        &self,
        request: Request<BatchAssignRolesRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.batch_assign_roles(Some(auth.user_id), req.user_ids, req.role_ids, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }
}
