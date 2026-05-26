//! Warehouse gRPC Handler — 委托给 abt-core WarehouseService

use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::types::ServiceContext;
use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_warehouse_service_server::AbtWarehouseService as GrpcWarehouseService, *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt_macros::require_permission;

pub struct WarehouseHandler;

impl WarehouseHandler {
    pub fn new() -> Self { Self }
}

impl Default for WarehouseHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpcWarehouseService for WarehouseHandler {
    #[require_permission(Resource::Warehouse, Action::Read)]
    async fn list_warehouses(&self, _request: Request<Empty>) -> GrpcResult<WarehouseListResponse> {
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.list(ctx, Default::default(), 1, 9999)
            .await.map_err(domain_to_status)?;

        Ok(Response::new(WarehouseListResponse {
            items: result.items.into_iter().map(|w| w.into()).collect(),
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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let warehouse = srv.get(ctx, req.warehouse_id)
            .await.map_err(domain_to_status)?;

        Ok(Response::new(warehouse.into()))
    }

    #[require_permission(Resource::Warehouse, Action::Write)]
    async fn create_warehouse(
        &self,
        request: Request<CreateWarehouseRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create(ctx, abt_core::wms::warehouse::CreateWarehouseReq {
            code: req.warehouse_code,
            name: req.warehouse_name,
            warehouse_type: abt_core::wms::WarehouseType::RawMaterial,
            address: if req.address.is_empty() { None } else { Some(req.address) },
            manager_id: None,
            is_virtual: false,
            remark: String::new(),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Warehouse, Action::Write)]
    async fn update_warehouse(
        &self,
        request: Request<UpdateWarehouseRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(ctx, req.warehouse_id, abt_core::wms::warehouse::UpdateWarehouseReq {
            name: Some(req.warehouse_name),
            address: if req.address.is_empty() { None } else { Some(req.address) },
            ..Default::default()
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Warehouse, Action::Write)]
    async fn update_warehouse_status(
        &self,
        request: Request<UpdateWarehouseStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let status = if req.is_active {
            abt_core::wms::WarehouseStatus::Active
        } else {
            abt_core::wms::WarehouseStatus::Inactive
        };

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(ctx, req.warehouse_id, abt_core::wms::warehouse::UpdateWarehouseReq {
            status: Some(status),
            ..Default::default()
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Warehouse, Action::Delete)]
    async fn delete_warehouse(
        &self,
        request: Request<DeleteWarehouseRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        if req.hard_delete {
            // abt-core WarehouseService 仅支持软删除，hard_delete 暂忽略并记录警告
            tracing::warn!(warehouse_id = req.warehouse_id, "hard_delete requested but abt-core only supports soft delete");
        }
        srv.delete(ctx, req.warehouse_id).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
