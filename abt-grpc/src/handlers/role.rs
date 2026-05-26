//! Role gRPC Handler — 委托给 abt-core RoleService

use abt_core::shared::identity::RoleService;
use abt_core::shared::types::ServiceContext;
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    role_service_server::RoleService as GrpcRoleService,
    *,
};
use crate::handlers::{domain_to_status, empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;

pub struct RoleHandler;

impl RoleHandler {
    pub fn new() -> Self {
        Self
    }

    /// 权限变更后刷新缓存
    async fn refresh_permission_cache(state: &AppState) -> Result<(), tonic::Status> {
        crate::server::get_permission_cache()
            .load(&state.core_pool())
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }
}

impl Default for RoleHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcRoleService for RoleHandler {
    #[require_permission(Resource::Role, Action::Write)]
    async fn create_role(
        &self,
        request: Request<CreateRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let role = srv
            .create_role(
                ctx,
                &req.role_name,
                &req.role_code,
                empty_to_none(req.description).as_deref(),
                None,
            )
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch with permissions to return full response
        let mut tx2 = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx2 = ServiceContext::new(&mut tx2, auth.user_id);
        let role_with_perms = srv
            .get_role_with_permissions(ctx2, role.role_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(role_with_perms.into()))
    }

    #[require_permission(Resource::Role, Action::Write)]
    async fn update_role(
        &self,
        request: Request<UpdateRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update_role(
            ctx,
            req.role_id,
            &req.role_name,
            empty_to_none(req.description).as_deref(),
        )
        .await
        .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the updated role with permissions to return
        let mut tx2 = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;
        let ctx2 = ServiceContext::new(&mut tx2, auth.user_id);
        let role_with_perms = srv
            .get_role_with_permissions(ctx2, req.role_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(role_with_perms.into()))
    }

    #[require_permission(Resource::Role, Action::Delete)]
    async fn delete_role(
        &self,
        request: Request<DeleteRoleRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete_role(ctx, req.role_id)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // 刷新权限缓存
        Self::refresh_permission_cache(&state).await?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Role, Action::Read)]
    async fn get_role(
        &self,
        request: Request<GetRoleRequest>,
    ) -> GrpcResult<RoleResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let role_with_perms = srv
            .get_role_with_permissions(ctx, req.role_id)
            .await
            .map_err(domain_to_status)?;

        Ok(Response::new(role_with_perms.into()))
    }

    #[require_permission(Resource::Role, Action::Read)]
    async fn list_roles(
        &self,
        request: Request<Empty>,
    ) -> GrpcResult<RoleListResponse> {
        let _req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let roles = srv.list_roles(ctx).await.map_err(domain_to_status)?;

        Ok(Response::new(RoleListResponse {
            roles: roles.into_iter().map(|r| r.into()).collect(),
        }))
    }

    #[require_permission(Resource::Role, Action::Write)]
    async fn assign_permissions(
        &self,
        request: Request<AssignPermissionsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let resource_actions: Vec<(String, String)> = req
            .permissions
            .iter()
            .map(|p| (p.resource_code.clone(), p.action_code.clone()))
            .collect();

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.assign_permissions(ctx, req.role_id, resource_actions)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // 刷新权限缓存
        Self::refresh_permission_cache(&state).await?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Role, Action::Write)]
    async fn remove_permissions(
        &self,
        request: Request<RemovePermissionsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.role_service();

        let mut tx = state
            .begin_core_transaction()
            .await
            .map_err(error::err_to_status)?;

        let resource_actions: Vec<(String, String)> = req
            .permissions
            .iter()
            .map(|p| (p.resource_code.clone(), p.action_code.clone()))
            .collect();

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.remove_permissions(ctx, req.role_id, resource_actions)
            .await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // 刷新权限缓存
        Self::refresh_permission_cache(&state).await?;

        Ok(Response::new(Empty {}))
    }
}
