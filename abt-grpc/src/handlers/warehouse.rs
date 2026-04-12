//! Warehouse gRPC Handler
use crate::generated::abt::v1::{
    abt_warehouse_service_server::AbtWarehouseService as GrpcWarehouseService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;
use common::error;
use tonic::{Request, Response};

// Import trait to bring methods into scope
use abt::WarehouseService;

pub struct WarehouseHandler;

impl WarehouseHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WarehouseHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcWarehouseService for WarehouseHandler {
    #[require_permission(Resource::Warehouse, Action::Read)]
    async fn list_warehouses(&self, _request: Request<Empty>) -> GrpcResult<WarehouseListResponse> {
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let warehouses = srv.list_all().await.map_err(error::err_to_status)?;

        Ok(Response::new(WarehouseListResponse {
            items: warehouses.into_iter().map(|w| w.into()).collect(),
        }))
    }

    #[require_permission(Resource::Warehouse, Action::Read)]
    async fn get_warehouse(
        &self,
        request: Request<GetWarehouseRequest>,
    ) -> GrpcResult<WarehouseResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let warehouse = srv
            .get_by_id(req.warehouse_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Warehouse", &req.warehouse_id.to_string()))?;

        Ok(Response::new(warehouse.into()))
    }

    #[require_permission(Resource::Warehouse, Action::Write)]
    async fn create_warehouse(
        &self,
        request: Request<CreateWarehouseRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateWarehouseRequest {
            warehouse_name: req.warehouse_name,
            warehouse_code: req.warehouse_code,
        };

        let id = srv
            .create(create_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Warehouse, Action::Write)]
    async fn update_warehouse(
        &self,
        request: Request<UpdateWarehouseRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateWarehouseRequest {
            warehouse_name: req.warehouse_name,
            warehouse_code: None,
            status: abt::WarehouseStatus::Active,
        };

        srv.update(req.warehouse_id, update_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Warehouse, Action::Write)]
    async fn update_warehouse_status(
        &self,
        request: Request<UpdateWarehouseStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let status = if req.is_active {
            abt::WarehouseStatus::Active
        } else {
            abt::WarehouseStatus::Inactive
        };

        let update_req = abt::UpdateWarehouseRequest {
            warehouse_name: String::new(),
            warehouse_code: None,
            status,
        };

        srv.update(req.warehouse_id, update_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Warehouse, Action::Delete)]
    async fn delete_warehouse(
        &self,
        request: Request<DeleteWarehouseRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let deleted = srv
            .delete(req.warehouse_id, req.hard_delete, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: deleted }))
    }
}
