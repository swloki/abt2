//! Role gRPC Handler

use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{
    role_service_server::RoleService as GrpcRoleService,
    *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

use abt::RoleService;
use crate::interceptors::auth::extract_auth;

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
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "write").map_err(|e| Status::permission_denied(e))?;
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

        let role_id = srv.create(Some(auth.user_id), create_req, &mut tx).await
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
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let update_req = abt::UpdateRoleRequest {
            role_name: if req.role_name.is_empty() { None } else { Some(req.role_name) },
            description: if req.description.is_empty() { None } else { Some(req.description) },
        };

        srv.update(Some(auth.user_id), req.role_id, update_req, &mut tx).await
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
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "delete").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.delete(Some(auth.user_id), req.role_id, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn get_role(
        &self,
        request: Request<GetRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "read").map_err(|e| Status::permission_denied(e))?;
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
        request: Request<Empty>,
    ) -> GrpcResult<RoleListResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "read").map_err(|e| Status::permission_denied(e))?;
        let _req = request.into_inner();
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
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let resource_actions: Vec<(String, String)> = req.permissions.iter()
            .map(|p| (p.resource_code.clone(), p.action_code.clone()))
            .collect();
        srv.assign_permissions(Some(auth.user_id), req.role_id, resource_actions, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn remove_permissions(
        &self,
        request: Request<RemovePermissionsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("role", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let resource_actions: Vec<(String, String)> = req.permissions.iter()
            .map(|p| (p.resource_code.clone(), p.action_code.clone()))
            .collect();
        srv.remove_permissions(Some(auth.user_id), req.role_id, resource_actions, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }
}
