//! H3Yun Sync gRPC Handler

use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_sync_service_server::AbtSyncService as GrpcSyncService, *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

pub struct SyncHandler;

impl SyncHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SyncHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcSyncService for SyncHandler {
    async fn sync_product(
        &self,
        request: Request<SyncProductRequest>,
    ) -> GrpcResult<SyncResponse> {
        let req = request.into_inner();
        if req.product_id <= 0 {
            return Err(error::validation("product_id", "产品 ID 无效"));
        }

        let sender = abt::h3yun::get_sync_event_sender().clone();
        let event = abt::h3yun::models::SyncEvent {
            entity_type: abt::h3yun::models::EntityType::Product,
            entity_id: req.product_id,
            priority: abt::h3yun::models::Priority::Normal,
        };

        sender
            .send(event)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("Failed to send sync event: {e}")))?;

        Ok(Response::new(SyncResponse {
            processed: 1,
            succeeded: 0,
            message: "Sync event queued".to_string(),
        }))
    }

    async fn sync_all_products(
        &self,
        _request: Request<SyncAllRequest>,
    ) -> GrpcResult<SyncResponse> {
        let state = AppState::get().await;
        let products: Vec<abt::models::Product> = {
            use abt::service::ProductService;
            state
                .product_service()
                .query(abt::models::ProductQuery::default())
                .await
                .map_err(error::err_to_status)?
                .0
        };

        let sender = abt::h3yun::get_sync_event_sender().clone();
        let mut queued = 0i32;

        for product in &products {
            let event = abt::h3yun::models::SyncEvent {
                entity_type: abt::h3yun::models::EntityType::Product,
                entity_id: product.product_id,
                priority: abt::h3yun::models::Priority::Low,
            };

            if sender.send(event).await.is_ok() {
                queued += 1;
            }
        }

        Ok(Response::new(SyncResponse {
            processed: queued,
            succeeded: 0,
            message: format!("{queued} sync events queued"),
        }))
    }

    async fn sync_inventory(
        &self,
        request: Request<SyncInventoryRequest>,
    ) -> GrpcResult<SyncResponse> {
        let req = request.into_inner();
        if req.product_id <= 0 {
            return Err(error::validation("product_id", "产品 ID 无效"));
        }

        let state = AppState::get().await;
        let inventories: Vec<abt::models::InventoryDetail> = {
            use abt::service::InventoryService;
            state
                .inventory_service()
                .get_by_product(req.product_id)
                .await
                .map_err(error::err_to_status)?
        };

        let sender = abt::h3yun::get_sync_event_sender().clone();
        let mut queued = 0i32;

        for inv in &inventories {
            let event = abt::h3yun::models::SyncEvent {
                entity_type: abt::h3yun::models::EntityType::Inventory,
                entity_id: inv.inventory_id,
                priority: abt::h3yun::models::Priority::Normal,
            };

            if sender.send(event).await.is_ok() {
                queued += 1;
            }
        }

        Ok(Response::new(SyncResponse {
            processed: queued,
            succeeded: 0,
            message: format!("{queued} inventory sync events queued"),
        }))
    }

    async fn reconcile(
        &self,
        request: Request<ReconcileRequest>,
    ) -> GrpcResult<ReconcileResponse> {
        let req = request.into_inner();
        let entity_type = match req.entity_type.as_str() {
            "product" => abt::h3yun::models::EntityType::Product,
            "inventory" => abt::h3yun::models::EntityType::Inventory,
            _ => return Err(error::validation("entity_type", "必须是 product 或 inventory")),
        };

        let state = AppState::get().await;
        let client = abt::h3yun::client::H3YunClient::new();
        let schema_code = match entity_type {
            abt::h3yun::models::EntityType::Product => abt::h3yun::models::schema::PRODUCT,
            abt::h3yun::models::EntityType::Inventory => abt::h3yun::models::schema::WAREHOUSE,
        };

        let remote_items = client
            .query_list(schema_code)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("H3Yun query failed: {e}")))?;

        let pool = state.pool();
        let local_mappings = abt::h3yun::sync_state::SyncStateRepo::find_all_by_type(
            &pool,
            entity_type,
        )
        .await
        .map_err(error::err_to_status)?;

        let mut drifts = Vec::new();

        let remote_object_ids: std::collections::HashSet<&str> = remote_items
            .iter()
            .filter_map(|item| item.get("ObjectId").and_then(|v| v.as_str()))
            .collect();

        for mapping in &local_mappings {
            if let Some(ref object_id) = mapping.h3yun_object_id {
                if !remote_object_ids.contains(object_id.as_str()) {
                    drifts.push(DriftItem {
                        entity_type: entity_type.as_str().to_string(),
                        entity_id: mapping.entity_id,
                        drift_type: "sync_lost".to_string(),
                        detail: format!(
                            "Local mapping exists (ObjectId: {object_id}) but not found in H3Yun"
                        ),
                    });
                }
            }
        }

        let local_object_ids: std::collections::HashSet<&str> = local_mappings
            .iter()
            .filter_map(|m| m.h3yun_object_id.as_deref())
            .collect();

        for item in &remote_items {
            if let Some(object_id) = item.get("ObjectId").and_then(|v| v.as_str()) {
                if !local_object_ids.contains(object_id) {
                    drifts.push(DriftItem {
                        entity_type: entity_type.as_str().to_string(),
                        entity_id: 0,
                        drift_type: "ghost_record".to_string(),
                        detail: format!(
                            "H3Yun record (ObjectId: {object_id}) has no local mapping"
                        ),
                    });
                }
            }
        }

        Ok(Response::new(ReconcileResponse { drifts }))
    }
}
