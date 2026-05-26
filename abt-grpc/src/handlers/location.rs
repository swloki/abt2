//! Location gRPC Handler — 委托给 abt-core WarehouseService (Bin 兼容)

use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::types::ServiceContext;
use common::error;
use tonic::{Request, Response};
use crate::generated::abt::v1::{
    abt_location_service_server::AbtLocationService as GrpcLocationService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

pub struct LocationHandler;

impl LocationHandler {
    pub fn new() -> Self { Self }
}

impl Default for LocationHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpcLocationService for LocationHandler {
    #[require_permission(Resource::Location, Action::Read)]
    async fn list_locations_by_warehouse(
        &self,
        request: Request<ListLocationsByWarehouseRequest>,
    ) -> GrpcResult<LocationListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.list_bins_by_warehouse(
            ctx,
            req.warehouse_id,
            req.keyword,
            req.is_active,
            req.page.unwrap_or(1),
            req.page_size.unwrap_or(20),
        ).await.map_err(domain_to_status)?;

        Ok(Response::new(LocationListResponse {
            items: result.items.into_iter().map(|b| b.into()).collect(),
            total: result.total,
            page: result.page,
            page_size: result.page_size,
        }))
    }

    #[require_permission(Resource::Location, Action::Read)]
    async fn list_all_locations(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<LocationWithWarehouseListResponse> {
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let bins = srv.list_all_bins_with_warehouse(ctx)
            .await.map_err(domain_to_status)?;

        Ok(Response::new(LocationWithWarehouseListResponse {
            items: bins.into_iter().map(|bw| bw.into()).collect(),
        }))
    }

    #[require_permission(Resource::Location, Action::Read)]
    async fn get_location(
        &self,
        request: Request<GetLocationRequest>,
    ) -> GrpcResult<LocationResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let bw = srv.get_bin_with_warehouse(ctx, req.location_id)
            .await.map_err(domain_to_status)?;

        Ok(Response::new(bw.into()))
    }

    #[require_permission(Resource::Location, Action::Write)]
    async fn create_location(
        &self,
        request: Request<CreateLocationRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        // 查找或创建默认库区
        let zone = srv.get_or_create_default_zone(ctx, req.warehouse_id)
            .await.map_err(domain_to_status)?;

        // 重新创建 ctx（ServiceContext 不实现 Copy）
        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create_bin(ctx, zone.id, abt_core::wms::warehouse::CreateBinReq {
            code: req.location_code,
            name: req.location_name,
            row_no: None,
            column_no: None,
            layer_no: None,
            capacity_limit: None,
            allowed_product_types: None,
            temperature_req: None,
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Location, Action::Write)]
    async fn update_location(
        &self,
        request: Request<UpdateLocationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update_bin(ctx, req.location_id, abt_core::wms::warehouse::UpdateBinReq {
            name: Some(req.location_name).filter(|s| !s.is_empty()),
            ..Default::default()
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Location, Action::Write)]
    async fn update_location_status(
        &self,
        request: Request<UpdateLocationStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let status = if req.is_active {
            abt_core::wms::BinStatus::Empty
        } else {
            abt_core::wms::BinStatus::Disabled
        };

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update_bin(ctx, req.location_id, abt_core::wms::warehouse::UpdateBinReq {
            status: Some(status),
            ..Default::default()
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Location, Action::Delete)]
    async fn delete_location(
        &self,
        request: Request<DeleteLocationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        if req.hard_delete {
            tracing::warn!(location_id = req.location_id, "hard_delete requested but abt-core only supports soft delete");
        }

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete_bin(ctx, req.location_id).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    // ── 库存统计 ──────────────────────────────────

    #[require_permission(Resource::Location, Action::Read)]
    async fn get_warehouse_inventory_stats(
        &self,
        request: Request<GetWarehouseInventoryStatsRequest>,
    ) -> GrpcResult<WarehouseInventoryStatsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, 0);

        let stats = srv.get_warehouse_inventory_stats(ctx, req.warehouse_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(stats.into()))
    }

    #[require_permission(Resource::Location, Action::Read)]
    async fn get_location_inventory_stats(
        &self,
        request: Request<GetLocationInventoryStatsRequest>,
    ) -> GrpcResult<LocationInventoryStatsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, 0);

        let stats = srv.get_bin_inventory_stats(ctx, req.location_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(stats.into()))
    }

    #[require_permission(Resource::Location, Action::Read)]
    async fn list_location_stats_by_warehouse(
        &self,
        request: Request<ListLocationStatsByWarehouseRequest>,
    ) -> GrpcResult<LocationStatsListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, 0);

        let result = srv.list_bin_stats_by_warehouse(
            ctx,
            req.warehouse_id,
            req.page.unwrap_or(1),
            req.page_size.unwrap_or(20),
        ).await
            .map_err(domain_to_status)?;

        Ok(Response::new(LocationStatsListResponse {
            items: result.items.into_iter().map(|s| s.into()).collect(),
            total: result.total as u64,
            page: result.page,
            page_size: result.page_size,
        }))
    }

    #[require_permission(Resource::Location, Action::Read)]
    async fn search_locations(
        &self,
        request: Request<SearchLocationsRequest>,
    ) -> GrpcResult<SearchLocationsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.warehouse_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.search_bins_with_warehouse(
            ctx,
            req.keyword,
            req.is_active,
            req.warehouse_id,
            req.page.unwrap_or(1),
            req.page_size.unwrap_or(20),
        ).await.map_err(domain_to_status)?;

        Ok(Response::new(SearchLocationsResponse {
            items: result.items.into_iter().map(|bw| bw.into()).collect(),
            total: result.total,
            page: result.page,
            page_size: result.page_size,
        }))
    }
}
