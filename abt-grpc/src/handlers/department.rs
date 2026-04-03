//! Department gRPC Handler

use tonic::{Request, Response};
use crate::generated::abt::v1::{
    department_service_server::DepartmentService as GrpcDepartmentService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use common::error;

use abt::DepartmentService;

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
    async fn create_department(
        &self,
        request: Request<CreateDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|_| error::forbidden("department", "write"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateDepartmentRequest {
            department_name: req.department_name,
            department_code: req.department_code,
            description: if req.description.is_empty() { None } else { Some(req.description) },
        };

        let department_id = srv.create(Some(auth.user_id), create_req, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the created department to return
        let department = srv.get(department_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Department", &department_id.to_string()))?;

        Ok(Response::new(department.into()))
    }

    async fn update_department(
        &self,
        request: Request<UpdateDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|_| error::forbidden("department", "write"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateDepartmentRequest {
            department_name: if req.department_name.is_empty() { None } else { Some(req.department_name) },
            description: if req.description.is_empty() { None } else { Some(req.description) },
            is_active: Some(req.is_active),
        };

        srv.update(Some(auth.user_id), req.department_id, update_req, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the updated department to return
        let department = srv.get(req.department_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Department", &req.department_id.to_string()))?;

        Ok(Response::new(department.into()))
    }

    async fn delete_department(
        &self,
        request: Request<DeleteDepartmentRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "delete").map_err(|_| error::forbidden("department", "delete"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(Some(auth.user_id), req.department_id, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    async fn get_department(
        &self,
        request: Request<GetDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "read").map_err(|_| error::forbidden("department", "read"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let department = srv.get(req.department_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Department", &req.department_id.to_string()))?;

        Ok(Response::new(department.into()))
    }

    async fn list_departments(
        &self,
        request: Request<ListDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "read").map_err(|_| error::forbidden("department", "read"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let departments = srv.list(req.include_inactive).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }

    async fn assign_departments(
        &self,
        request: Request<AssignDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|_| error::forbidden("department", "write"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.assign_departments(Some(auth.user_id), req.user_id, req.department_ids, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    async fn remove_departments(
        &self,
        request: Request<RemoveDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|_| error::forbidden("department", "write"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.remove_departments(Some(auth.user_id), req.user_id, req.department_ids, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    async fn get_user_departments(
        &self,
        request: Request<GetUserDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "read").map_err(|_| error::forbidden("department", "read"))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let departments = srv.get_user_departments(req.user_id).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }
}
