//! Department gRPC Handler

use crate::generated::abt::v1::{
    department_service_server::DepartmentService as GrpcDepartmentService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use abt::DepartmentService;

/// Convert stored resource code strings back to proto Resource i32 values.
/// DB stores lowercase ("product", "labor_process"), proto str_name is SCREAMING_SNAKE.
fn strings_to_resources(codes: &[String]) -> Vec<i32> {
    codes
        .iter()
        .filter_map(|code| Resource::from_str_name(&code.to_uppercase()))
        .map(|r| r as i32)
        .collect()
}

/// Convert proto DeptRoleAssignment to model DeptRole.
fn to_dept_roles(assignments: Vec<DeptRoleAssignment>) -> Vec<abt::DeptRole> {
    assignments
        .into_iter()
        .map(|a| abt::DeptRole {
            department_id: a.department_id,
            role_id: a.role_id,
        })
        .collect()
}

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
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateDepartmentRequest {
            department_name: req.department_name,
            department_code: req.department_code,
            description: if req.description.is_empty() {
                None
            } else {
                Some(req.description)
            },
        };

        let department_id = srv
            .create(create_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the created department to return
        let department = srv
            .get(department_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Department", &department_id.to_string()))?;

        Ok(Response::new(department.into()))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn update_department(
        &self,
        request: Request<UpdateDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateDepartmentRequest {
            department_name: if req.department_name.is_empty() {
                None
            } else {
                Some(req.department_name)
            },
            description: if req.description.is_empty() {
                None
            } else {
                Some(req.description)
            },
            is_active: Some(req.is_active),
        };

        srv.update(req.department_id, update_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        // Fetch the updated department to return
        let department = srv
            .get(req.department_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Department", &req.department_id.to_string()))?;

        Ok(Response::new(department.into()))
    }

    #[require_permission(Resource::Department, Action::Delete)]
    async fn delete_department(
        &self,
        request: Request<DeleteDepartmentRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete(req.department_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn get_department(
        &self,
        request: Request<GetDepartmentRequest>,
    ) -> GrpcResult<DepartmentResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let department = srv
            .get(req.department_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Department", &req.department_id.to_string()))?;

        Ok(Response::new(department.into()))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn list_departments(
        &self,
        request: Request<ListDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let departments = srv
            .list(req.include_inactive)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn assign_departments(
        &self,
        request: Request<AssignDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.assign_departments(req.user_id, req.department_ids, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn remove_departments(
        &self,
        request: Request<RemoveDepartmentsRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.remove_departments(req.user_id, req.department_ids, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn get_user_departments(
        &self,
        request: Request<GetUserDepartmentsRequest>,
    ) -> GrpcResult<DepartmentListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let departments = srv
            .get_user_departments(req.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(DepartmentListResponse {
            departments: departments.into_iter().map(|d| d.into()).collect(),
        }))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn set_department_resources(
        &self,
        request: Request<SetDepartmentResourcesRequest>,
    ) -> GrpcResult<SetDepartmentResourcesResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let resource_codes: Vec<String> = req
            .resources
            .iter()
            .filter_map(|&v| {
                let r = Resource::try_from(v).ok()?;
                Some(PermissionCode::code(&r).to_string())
            })
            .collect();

        let stored_codes = srv
            .set_department_resources(
                req.department_id,
                resource_codes,
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        let resources = strings_to_resources(&stored_codes);

        Ok(Response::new(SetDepartmentResourcesResponse { resources }))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn get_department_resources(
        &self,
        request: Request<GetDepartmentResourcesRequest>,
    ) -> GrpcResult<GetDepartmentResourcesResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let codes = srv
            .get_department_resources(req.department_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(GetDepartmentResourcesResponse {
            resources: strings_to_resources(&codes),
        }))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn assign_user_department_roles(
        &self,
        request: Request<AssignUserDepartmentRolesRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let assignments = to_dept_roles(req.assignments);

        srv.assign_user_dept_roles(req.user_id, assignments, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Write)]
    async fn remove_user_department_roles(
        &self,
        request: Request<RemoveUserDepartmentRolesRequest>,
    ) -> GrpcResult<Empty> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let assignments = to_dept_roles(req.assignments);

        srv.remove_user_dept_roles(req.user_id, assignments, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(Empty {}))
    }

    #[require_permission(Resource::Department, Action::Read)]
    async fn get_user_department_roles(
        &self,
        request: Request<GetUserDepartmentRolesRequest>,
    ) -> GrpcResult<UserDepartmentRolesResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.department_service();

        let roles = srv
            .get_user_dept_roles(req.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(UserDepartmentRolesResponse {
            roles: roles
                .into_iter()
                .map(|r| DeptRoleDetail {
                    department_id: r.department_id,
                    department_name: r.department_name,
                    role_id: r.role_id,
                    role_name: r.role_name,
                })
                .collect(),
        }))
    }
}
