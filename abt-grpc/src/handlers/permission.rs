//! Permission gRPC Handler

use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{
    permission_service_server::PermissionService as GrpcPermissionService,
    *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

use abt::PermissionService;

pub struct PermissionHandler;

impl PermissionHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PermissionHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcPermissionService for PermissionHandler {
    async fn get_user_permissions(
        &self,
        request: Request<GetUserPermissionsRequest>,
    ) -> GrpcResult<UserPermissionsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let permissions = srv.get_user_permissions(req.user_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UserPermissionsResponse {
            permissions: permissions.into_iter().map(|p| p.into()).collect(),
        }))
    }

    async fn check_permission(
        &self,
        request: Request<CheckPermissionRequest>,
    ) -> GrpcResult<CheckPermissionResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let has_permission = srv.check_permission(
            req.user_id,
            &req.resource_code,
            &req.action_code,
        ).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CheckPermissionResponse {
            has_permission,
        }))
    }

    async fn list_resources(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<ResourceListResponse> {
        let state = AppState::get().await;
        let srv = state.permission_service();

        let resource_groups = srv.list_resources().await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ResourceListResponse {
            groups: resource_groups.into_iter().map(|g| g.into()).collect(),
        }))
    }

    async fn list_permissions(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<PermissionListResponse> {
        let state = AppState::get().await;
        let srv = state.permission_service();

        let permission_groups = srv.list_permissions().await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(PermissionListResponse {
            groups: permission_groups.into_iter().map(|g| g.into()).collect(),
        }))
    }

    async fn list_audit_logs(
        &self,
        request: Request<ListAuditLogsRequest>,
    ) -> GrpcResult<AuditLogListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let logs = srv.list_audit_logs(req.limit, req.offset).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(AuditLogListResponse {
            logs: logs.into_iter().map(|l| l.into()).collect(),
        }))
    }
}
