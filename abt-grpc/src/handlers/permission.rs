use abt_core::shared::identity::{PermissionService, RESOURCE_ACTION_DEFS, ResourceActionDef};
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
        let auth = extract_auth(&request)?;
        let _req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let codes = srv
            .get_user_permissions(&auth.role_ids)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        let permissions: Vec<PermissionInfo> = codes
            .iter()
            .filter_map(|code| {
                let (resource_code, action_code) = code.split_once(':')?;
                RESOURCE_ACTION_DEFS
                    .iter()
                    .find(|r| r.resource_code == resource_code && r.action == action_code)
                    .map(|r| PermissionInfo {
                        permission_id: 0,
                        permission_name: format!("{}-{}", r.resource_name, r.action_name),
                        resource: Some(ResourceInfo {
                            resource_id: 0,
                            resource_name: r.resource_name.to_string(),
                            resource_code: to_proto_code(r.resource_code),
                            group_name: r.resource_name.to_string(),
                        }),
                        action_code: to_proto_code(r.action),
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
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        let resource = Resource::try_from(req.resource)
            .map_err(|_| tonic::Status::invalid_argument("Invalid resource value"))?;
        let action = Action::try_from(req.action)
            .map_err(|_| tonic::Status::invalid_argument("Invalid action value"))?;

        let state = AppState::get().await;
        let srv = state.permission_service();

        let has_permission = srv
            .check_permission(auth.is_super_admin(), &auth.role_ids, &resource.code(), &action.code())
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(CheckPermissionResponse { has_permission }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_resources(&self, _request: Request<Empty>) -> GrpcResult<ResourceListResponse> {
        let groups = group_resources(RESOURCE_ACTION_DEFS);

        Ok(Response::new(ResourceListResponse { groups }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_user_resources(
        &self,
        request: Request<ListUserResourcesRequest>,
    ) -> GrpcResult<ResourceListResponse> {
        let auth = extract_auth(&request)?;
        let _req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.permission_service();

        let user_codes = srv
            .get_user_permissions(&auth.role_ids)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        let user_resources: Vec<_> = RESOURCE_ACTION_DEFS
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
        let groups = group_permissions(RESOURCE_ACTION_DEFS);

        Ok(Response::new(PermissionListResponse { groups }))
    }

    #[require_permission(Resource::Permission, Action::Read)]
    async fn list_audit_logs(
        &self,
        _request: Request<ListAuditLogsRequest>,
    ) -> GrpcResult<AuditLogListResponse> {
        // Stub: audit log listing is not yet implemented in abt-core.
        // Returns an empty list until the audit log query service is available.
        Ok(Response::new(AuditLogListResponse { logs: vec![] }))
    }
}

/// Convert internal lowercase codes to SCREAMING_SNAKE_CASE to match proto enum names.
fn to_proto_code(code: &str) -> String {
    code.to_uppercase()
}

fn group_resources(resources: &[ResourceActionDef]) -> Vec<ResourceGroup> {
    let mut groups: std::collections::HashMap<&str, Vec<ResourceInfo>> =
        std::collections::HashMap::new();
    for r in resources {
        groups
            .entry(r.resource_name)
            .or_default()
            .push(ResourceInfo {
                resource_id: 0,
                resource_name: r.resource_name.to_string(),
                resource_code: to_proto_code(r.resource_code),
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

fn group_resources_by_refs(resources: &[&ResourceActionDef]) -> Vec<ResourceGroup> {
    let mut groups: std::collections::HashMap<&str, Vec<ResourceInfo>> =
        std::collections::HashMap::new();
    for r in resources {
        groups
            .entry(r.resource_name)
            .or_default()
            .push(ResourceInfo {
                resource_id: 0,
                resource_name: r.resource_name.to_string(),
                resource_code: to_proto_code(r.resource_code),
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

fn group_permissions(resources: &[ResourceActionDef]) -> Vec<PermissionGroup> {
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
                    resource_code: to_proto_code(r.resource_code),
                    group_name: r.resource_name.to_string(),
                }),
                action_code: to_proto_code(r.action),
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
