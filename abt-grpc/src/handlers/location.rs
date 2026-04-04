//! Location gRPC Handler

use common::error;
use tonic::{Request, Response};
use crate::generated::abt::v1::{
    abt_location_service_server::AbtLocationService as GrpcLocationService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;

// Import trait to bring methods into scope
use abt::LocationService;
use abt::WarehouseService;

pub struct LocationHandler;

impl LocationHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocationHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcLocationService for LocationHandler {
    #[require_permission("location", "read")]
    async fn list_locations_by_warehouse(
        &self,
        request: Request<ListLocationsByWarehouseRequest>,
    ) -> GrpcResult<LocationListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let locations = srv.list_by_warehouse(req.warehouse_id).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(LocationListResponse {
            items: locations.into_iter().map(|l| l.into()).collect(),
        }))
    }

    #[require_permission("location", "read")]
    async fn list_all_locations(
        &self,
        request: Request<Empty>,
    ) -> GrpcResult<LocationWithWarehouseListResponse> {
        let state = AppState::get().await;
        let warehouse_srv = state.warehouse_service();

        // Get all warehouses
        let warehouses = warehouse_srv.list_all().await
            .map_err(error::err_to_status)?;

        // Build warehouse map for lookup
        let warehouse_map: std::collections::HashMap<i64, String> = warehouses
            .iter()
            .map(|w| (w.warehouse_id, w.warehouse_name.clone()))
            .collect();

        // Fetch all locations in parallel using futures
        let futures: Vec<_> = warehouses
            .into_iter()
            .map(|w| {
                let srv = state.location_service();
                async move {
                    srv.list_by_warehouse(w.warehouse_id).await
                        .map(|locs| (w.warehouse_id, locs))
                }
            })
            .collect();

        let results = futures::future::try_join_all(futures)
            .await
            .map_err(error::err_to_status)?;

        // Flatten and convert to response
        let all_locations: Vec<LocationWithWarehouseResponse> = results
            .into_iter()
            .flat_map(|(warehouse_id, locations)| {
                let warehouse_name = warehouse_map.get(&warehouse_id).cloned().unwrap_or_default();
                locations.into_iter().map(move |loc| LocationWithWarehouseResponse {
                    location_id: loc.location_id,
                    warehouse_id: loc.warehouse_id,
                    warehouse_name: warehouse_name.clone(),
                    location_code: loc.location_code,
                    location_name: loc.location_name.unwrap_or_default(),
                    location_type: String::new(),
                    is_active: loc.deleted_at.is_none(),
                })
            })
            .collect();

        Ok(Response::new(LocationWithWarehouseListResponse {
            items: all_locations,
        }))
    }

    #[require_permission("location", "read")]
    async fn get_location(
        &self,
        request: Request<GetLocationRequest>,
    ) -> GrpcResult<LocationResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let location = srv.get_by_id(req.location_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Location", &req.location_id.to_string()))?;

        Ok(Response::new(location.into()))
    }

    #[require_permission("location", "write")]
    async fn create_location(
        &self,
        request: Request<CreateLocationRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let create_req = abt::CreateLocationRequest {
            warehouse_id: req.warehouse_id,
            location_code: req.location_code,
            location_name: Some(req.location_name).filter(|s| !s.is_empty()),
            capacity: None,
        };

        let id = srv.create(create_req, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission("location", "write")]
    async fn update_location(
        &self,
        request: Request<UpdateLocationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let update_req = abt::UpdateLocationRequest {
            location_code: String::new(),
            location_name: Some(req.location_name).filter(|s| !s.is_empty()),
            capacity: None,
        };

        srv.update(req.location_id, update_req, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission("location", "delete")]
    async fn delete_location(
        &self,
        request: Request<DeleteLocationRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let deleted = srv.delete(req.location_id, req.hard_delete, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: deleted }))
    }

    #[require_permission("location", "read")]
    async fn get_warehouse_inventory_stats(
        &self,
        request: Request<GetWarehouseInventoryStatsRequest>,
    ) -> GrpcResult<WarehouseInventoryStatsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let stats = srv.get_warehouse_inventory_stats(req.warehouse_id).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(stats.into()))
    }

    #[require_permission("location", "read")]
    async fn get_location_inventory_stats(
        &self,
        request: Request<GetLocationInventoryStatsRequest>,
    ) -> GrpcResult<LocationInventoryStatsResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let stats = srv.get_location_inventory_stats(req.location_id).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(stats.into()))
    }

    #[require_permission("location", "read")]
    async fn list_location_stats_by_warehouse(
        &self,
        request: Request<ListLocationStatsByWarehouseRequest>,
    ) -> GrpcResult<LocationStatsListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.location_service();

        let result = srv.list_location_stats_by_warehouse(
            req.warehouse_id,
            req.page,
            req.page_size,
        ).await
            .map_err(error::err_to_status)?;

        Ok(Response::new(LocationStatsListResponse {
            items: result.items.into_iter().map(|s| s.into()).collect(),
            total: result.total as u64,
            page: result.page,
            page_size: result.page_size,
        }))
    }
}
