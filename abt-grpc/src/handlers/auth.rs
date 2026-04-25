//! Auth gRPC Handler

use tonic::{Request, Response};

use crate::generated::abt::v1::{
    auth_service_server::AuthService as GrpcAuthService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::{extract_auth, extract_user_id_from_header};
use crate::server::AppState;
use common::error;

use abt::{AuthService, UserService};

pub struct AuthHandler;

impl AuthHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuthHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcAuthService for AuthHandler {
    async fn login(&self, request: Request<LoginRequest>) -> GrpcResult<LoginResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.auth_service();

        let (token, expires_at, claims) = srv
            .login(&req.username, &req.password)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("disabled") {
                    error::unauthorized("User account is disabled")
                } else {
                    error::unauthorized("Invalid username or password")
                }
            })?;

        // 获取用户详情
        let user_srv = state.user_service();
        let user_with_roles = user_srv
            .get(claims.sub)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("User", &claims.sub.to_string()))?;

        Ok(Response::new(LoginResponse {
            token,
            expires_at,
            user: Some(user_with_roles.into()),
        }))
    }

    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> GrpcResult<TokenResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.auth_service();

        let (token, expires_at, _claims) = srv
            .refresh_token(&req.token)
            .await
            .map_err(|e| error::unauthorized(&e.to_string()))?;

        Ok(Response::new(TokenResponse { token, expires_at }))
    }

    async fn logout(&self, _request: Request<Empty>) -> GrpcResult<Empty> {
        // JWT 是无状态的，logout 由前端丢弃 token 即可
        Ok(Response::new(Empty {}))
    }

    async fn get_current_user(&self, request: Request<Empty>) -> GrpcResult<UserResponse> {
        let user_id = extract_user_id_from_header(&request)?;

        let state = AppState::get().await;
        let user_srv = state.user_service();

        let user_with_roles = user_srv
            .get(user_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("User", &user_id.to_string()))?;

        Ok(Response::new(user_with_roles.into()))
    }

    async fn list_resources(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<AuthResourceListResponse> {
        let state = AppState::get().await;
        let srv = state.auth_service();

        let resources = srv.list_resources();

        Ok(Response::new(AuthResourceListResponse {
            resources: resources
                .into_iter()
                .map(|r| AuthResourceAction {
                    resource_code: r.resource_code.to_string(),
                    resource_name: r.resource_name.to_string(),
                    description: r.description.to_string(),
                    action: r.action.to_string(),
                    action_name: r.action_name.to_string(),
                })
                .collect(),
        }))
    }

    async fn get_permissions_by_roles(
        &self,
        request: Request<GetPermissionsByRolesRequest>,
    ) -> GrpcResult<GetPermissionsByRolesResponse> {
        let auth = extract_auth(&request)?;
        let cache = abt::get_permission_cache();
        let mut permissions: Vec<String> = cache
            .get_merged_permissions(&auth.role_ids)
            .into_iter()
            .collect();
        permissions.sort();
        Ok(Response::new(GetPermissionsByRolesResponse { permissions }))
    }
}
