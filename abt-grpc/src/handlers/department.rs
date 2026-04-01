//! Department gRPC Handler

use tonic::{Request, Response, Status};
use crate::generated::abt::v1::{
    department_service_server::DepartmentService as GrpcDepartmentService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

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
        auth.check_permission("department", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let create_req = abt::CreateDepartmentRequest {
            department_name: req.department_name,
            department_code: req.department_code,
            description: if req.description.is_empty() { None } else { Some(req.description) },
        };

        let department_id = srv.create(Some(auth.user_id), create_req, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        // Fetch the created department to return
        let department = srv.get(department_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Department not found"))?;

        Ok(Response::new(department.into()))
    }

    async fn update_department(
        &self,
        request: Request<UpdateDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        let update_req = abt::UpdateDepartmentRequest {
            department_name: if req.department_name.is_empty() { None } else { Some(req.department_name) },
            description: if req.description.is_empty() { None } else { Some(req.description) },
            is_active: Some(req.is_active),
        };

        srv.update(Some(auth.user_id), req.department_id, update_req, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        // Fetch the updated department to return
        let department = srv.get(req.department_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Department not found"))?;

        Ok(Response::new(department.into()))
    }

    async fn delete_department(
        &self,
        request: Request<DeleteDepartmentRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "delete").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.delete(Some(auth.user_id), req.department_id, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn get_department(
        &self,
        request: Request<GetDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "read").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let department = srv.get(req.department_id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Department not found"))?;

        Ok(Response::new(department.into()))
    }

    async fn list_departments(
        &self,
        request: Request<ListDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "read").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let departments = srv.list(req.include_inactive).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }

    async fn assign_departments(
        &self,
        request: Request<AssignDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.assign_departments(Some(auth.user_id), req.user_id, req.department_ids, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn remove_departments(
        &self,
        request: Request<RemoveDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "write").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state.begin_transaction().await
            .map_err(|e| Status::internal(e.to_string()))?;

        srv.remove_departments(Some(auth.user_id), req.user_id, req.department_ids, &mut tx).await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn get_user_departments(
        &self,
        request: Request<GetUserDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("department", "read").map_err(|e| Status::permission_denied(e))?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let departments = srv.get_user_departments(req.user_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }
}
