//! Role gRPC Handler

use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{
    role_service_server::RoleService as GrpcRoleService,
    *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

use abt::RoleService;

pub struct RoleHandler;

impl RoleHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RoleHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcRoleService for RoleHandler {
    async fn create_role(
        &self,
        request: Request<CreateRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let create_req = abt::CreateRoleRequest {
            role_name: req.role_name,
            role_code: req.role_code,
            description: if req.description.is_empty() { None } else { Some(req.description) },
        };

        let role_id = srv.create(1, create_req, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        // Fetch the created role to return
        let role_with_perms = srv.get(role_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Role not found"))?;

        Ok(Response::new(role_with_perms.into()))
    }

    async fn update_role(
        &self,
        request: Request<UpdateRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let update_req = abt::UpdateRoleRequest {
            role_name: if req.role_name.is_empty() { None } else { Some(req.role_name) },
            description: if req.description.is_empty() { None } else { Some(req.description) },
        };

        srv.update(1, req.role_id, update_req, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        // Fetch the updated role to return
        let role_with_perms = srv.get(req.role_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Role not found"))?;

        Ok(Response::new(role_with_perms.into()))
    }

    async fn delete_role(
        &self,
        request: Request<DeleteRoleRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.delete(1, req.role_id, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn get_role(
        &self,
        request: Request<GetRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let role_with_perms = srv.get(req.role_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Role not found"))?;

        Ok(Response::new(role_with_perms.into()))
    }

    async fn list_roles(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<RoleListResponse> {
        let state = AppState::get().await;
        let srv = state.role_service();

        let roles = srv.list().await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(RoleListResponse {
            roles: roles.into_iter().map(|r| r.into()).collect(),
        }))
    }

    async fn assign_permissions(
        &self,
        request: Request<AssignPermissionsRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.assign_permissions(1, req.role_id, req.permission_ids, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn remove_permissions(
        &self,
        request: Request<RemovePermissionsRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.remove_permissions(1, req.role_id, req.permission_ids, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }
}
