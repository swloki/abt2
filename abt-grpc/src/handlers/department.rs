//! Department gRPC Handler

use abt_core::shared::identity::department_service::DepartmentService;
use abt_core::shared::types::context::ServiceContext;
use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    department_service_server::DepartmentService as GrpcDepartmentService, *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;

pub struct DepartmentHandler;

impl DepartmentHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DepartmentHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcDepartmentService for DepartmentHandler {
    #[require_permission(Resource::Department, Action::Write)]
    async fn create_department(
        &self,
        request: Request<CreateDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        let department = srv
            .create_department(
                ctx,
                &req.department_name,
                &req.department_code,
                empty_to_none(req.description).as_deref(),
            )
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(department.into()))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn update_department(
        &self,
        request: Request<UpdateDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        let department = srv
            .update_department(
                ctx,
                req.department_id,
                &req.department_name,
                empty_to_none(req.description).as_deref(),
            )
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(department.into()))
    }

    #[require_permission(Resource::Department, Action::Delete)]
    async fn delete_department(
        &self,
        request: Request<DeleteDepartmentRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        srv.delete_department(ctx, req.department_id)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn get_department(
        &self,
        request: Request<GetDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        let department = srv
            .get_department(ctx, req.department_id)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(department.into()))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn list_departments(
        &self,
        request: Request<ListDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let auth = extract_auth(&request)?;
        let _req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        let departments = srv
            .list_departments(ctx)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn assign_departments(
        &self,
        request: Request<AssignDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        srv.assign_departments(ctx, req.user_id, req.department_ids)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn remove_departments(
        &self,
        request: Request<RemoveDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        srv.remove_departments(ctx, req.user_id, req.department_ids)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn get_user_departments(
        &self,
        request: Request<GetUserDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut conn = state
            .core_pool()
            .acquire()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        let ctx = ServiceContext::new(&mut *conn, auth.user_id);

        let departments = srv
            .get_user_departments(ctx, req.user_id)
            .await
            .map_err(crate::handlers::domain_to_status)?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }
}
