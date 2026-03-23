//! User gRPC Handler

use crate::generated::abt::v1::{user_service_server::UserService as GrpcUserService, *};
use crate::handlers::GrpcResult;
use crate::server::AppState;
use tonic::{Request, Response, Status};

use abt::UserService;

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
    async fn create_user(&self, request: Request<CreateUserRequest>) -> GrpcResult<UserResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
            .create(1, create_req, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Fetch the created user to return
        let user_with_roles = srv
            .get(user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn update_user(&self, request: Request<UpdateUserRequest>) -> GrpcResult<UserResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let update_req = abt::UpdateUserRequest {
            display_name: if req.display_name.is_empty() {
                None
            } else {
                Some(req.display_name)
            },
            is_active: Some(req.is_active),
        };

        srv.update(1, req.user_id, update_req, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Fetch the updated user to return
        let user_with_roles = srv
            .get(req.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn delete_user(&self, request: Request<DeleteUserRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.delete(1, req.user_id, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn get_user(&self, request: Request<GetUserRequest>) -> GrpcResult<UserResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let user_with_roles = srv
            .get(req.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn list_users(&self, _request: Request<Empty>) -> GrpcResult<UserListResponse> {
        let state = AppState::get().await;
        let srv = state.user_service();

        let users = srv
            .list()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UserListResponse {
            users: users.into_iter().map(|u| u.into()).collect(),
        }))
    }

    async fn assign_roles(&self, request: Request<AssignRolesRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.assign_roles(1, req.user_id, req.role_ids, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn remove_roles(&self, request: Request<RemoveRolesRequest>) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.user_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.remove_roles(1, req.user_id, req.role_ids, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

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
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.batch_assign_roles(1, req.user_ids, req.role_ids, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }
}
