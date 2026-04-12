use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    permission_service_server::PermissionService as GrpcPermissionService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

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
    #[require_permission(Resource::Permission, Action::Read)]
    async fn get_user_permissions(
        &self,
        request: Request<GetUserPermissionsRequest>,
    ) -> GrpcResult<UserPermissionsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let codes = srv
            .get_user_permissions(req.user_id)
            .await
            .map_err(error::err_to_status)?;

        let all_resources = abt::collect_all_resources();
        let permissions: Vec<PermissionInfo> = codes
            .iter()
            .filter_map(|code| {
                let (resource_code, action_code) = code.split_once(':')?;
                all_resources
                    .iter()
                    .find(|r| r.resource_code == resource_code && r.action == action_code)
                    .map(|r| PermissionInfo {
                        permission_id: 0,
                        permission_name: format!("{}-{}", r.resource_name, r.action_name),
                        resource: Some(ResourceInfo {
                            resource_id: 0,
                            resource_name: r.resource_name.to_string(),
                            resource_code: r.resource_code.to_string(),
                            group_name: r.resource_name.to_string(),
                        }),
                        action_code: r.action.to_string(),
                        action_name: r.action_name.to_string(),
                    })
            })
            .collect();

        Ok(Response::new(UserPermissionsResponse { permissions }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn check_permission(
        &self,
        request: Request<CheckPermissionRequest>,
    ) -> GrpcResult<CheckPermissionResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let has_permission = srv
            .check_permission(req.user_id, &req.resource_code, &req.action_code)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(CheckPermissionResponse { has_permission }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_resources(&self, _request: Request<Empty>) -> GrpcResult<ResourceListResponse> {
        let all_resources = abt::collect_all_resources();
        let groups = group_resources(&all_resources);

        Ok(Response::new(ResourceListResponse { groups }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_user_resources(
        &self,
        request: Request<ListUserResourcesRequest>,
    ) -> GrpcResult<ResourceListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let user_codes = srv
            .get_user_permissions(req.user_id)
            .await
            .map_err(error::err_to_status)?;

        let all_resources = abt::collect_all_resources();
        let user_resources: Vec<_> = all_resources
            .iter()
            .filter(|r| {
                let code = format!("{}:{}", r.resource_code, r.action);
                user_codes.contains(&code)
            })
            .collect();

        let groups = group_resources_by_refs(&user_resources);

        Ok(Response::new(ResourceListResponse { groups }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_permissions(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<PermissionListResponse> {
        let all_resources = abt::collect_all_resources();
        let groups = group_permissions(&all_resources);

        Ok(Response::new(PermissionListResponse { groups }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_audit_logs(
        &self,
        request: Request<ListAuditLogsRequest>,
    ) -> GrpcResult<AuditLogListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let logs = srv
            .list_audit_logs(req.limit, req.offset)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(AuditLogListResponse {
            logs: logs.into_iter().map(|l| l.into()).collect(),
        }))
    }
}

fn group_resources(resources: &[abt::ResourceActionDef]) -> Vec<ResourceGroup> {
    let mut groups: std::collections::HashMap<&str, Vec<ResourceInfo>> =
        std::collections::HashMap::new();
    for r in resources {
        groups
            .entry(r.resource_name)
            .or_default()
            .push(ResourceInfo {
                resource_id: 0,
                resource_name: r.resource_name.to_string(),
                resource_code: r.resource_code.to_string(),
                group_name: r.resource_name.to_string(),
            });
    }
    groups
        .into_iter()
        .map(|(name, resources)| ResourceGroup {
            group_name: name.to_string(),
            resources,
        })
        .collect()
}

fn group_resources_by_refs(resources: &[&abt::ResourceActionDef]) -> Vec<ResourceGroup> {
    let mut groups: std::collections::HashMap<&str, Vec<ResourceInfo>> =
        std::collections::HashMap::new();
    for r in resources {
        groups
            .entry(r.resource_name)
            .or_default()
            .push(ResourceInfo {
                resource_id: 0,
                resource_name: r.resource_name.to_string(),
                resource_code: r.resource_code.to_string(),
                group_name: r.resource_name.to_string(),
            });
    }
    groups
        .into_iter()
        .map(|(name, resources)| ResourceGroup {
            group_name: name.to_string(),
            resources,
        })
        .collect()
}

fn group_permissions(resources: &[abt::ResourceActionDef]) -> Vec<PermissionGroup> {
    let mut groups: std::collections::HashMap<&str, Vec<PermissionInfo>> =
        std::collections::HashMap::new();
    for r in resources {
        groups
            .entry(r.resource_name)
            .or_default()
            .push(PermissionInfo {
                permission_id: 0,
                permission_name: format!("{}-{}", r.resource_name, r.action_name),
                resource: Some(ResourceInfo {
                    resource_id: 0,
                    resource_name: r.resource_name.to_string(),
                    resource_code: r.resource_code.to_string(),
                    group_name: r.resource_name.to_string(),
                }),
                action_code: r.action.to_string(),
                action_name: r.action_name.to_string(),
            });
    }
    groups
        .into_iter()
        .map(|(name, permissions)| PermissionGroup {
            group_name: name.to_string(),
            permissions,
        })
        .collect()
}
