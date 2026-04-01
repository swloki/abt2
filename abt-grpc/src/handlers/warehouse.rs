//! Warehouse gRPC Handler
use crate::generated::abt::v1::{
    abt_warehouse_service_server::AbtWarehouseService as GrpcWarehouseService, *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;
use crate::interceptors::auth::extract_auth;
use tonic::{Request, Response, Status};

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
    async fn list_warehouses(&self, request: Request<Empty>) -> GrpcResult<WarehouseListResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("warehouse", "read").map_err(|e| Status::permission_denied(e.to_string()))?;

        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let warehouses = srv
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(WarehouseListResponse {
            items: warehouses.into_iter().map(|w| w.into()).collect(),
        }))
    }

    async fn get_warehouse(
        &self,
        request: Request<GetWarehouseRequest>,
    ) -> GrpcResult<WarehouseResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("warehouse", "read").map_err(|e| Status::permission_denied(e.to_string()))?;

        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let warehouse = srv
            .get_by_id(req.warehouse_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Warehouse not found"))?;

        Ok(Response::new(warehouse.into()))
    }

    async fn create_warehouse(
        &self,
        request: Request<CreateWarehouseRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        auth.check_permission("warehouse", "write").map_err(|e| Status::permission_denied(e.to_string()))?;

        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let create_req = abt::CreateWarehouseRequest {
            warehouse_name: req.warehouse_name,
            warehouse_code: req.warehouse_code,
        };

        let id = srv
            .create(create_req, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_warehouse(
        &self,
        request: Request<UpdateWarehouseRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("warehouse", "write").map_err(|e| Status::permission_denied(e.to_string()))?;

        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let update_req = abt::UpdateWarehouseRequest {
            warehouse_name: req.warehouse_name,
            warehouse_code: None,
            status: abt::WarehouseStatus::Active,
        };

        srv.update(req.warehouse_id, update_req, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn update_warehouse_status(
        &self,
        request: Request<UpdateWarehouseStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("warehouse", "write").map_err(|e| Status::permission_denied(e.to_string()))?;

        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_warehouse(
        &self,
        request: Request<DeleteWarehouseRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        auth.check_permission("warehouse", "delete").map_err(|e| Status::permission_denied(e.to_string()))?;

        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let deleted = srv
            .delete(req.warehouse_id, req.hard_delete, &mut tx)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BoolResponse { value: deleted }))
    }
}
