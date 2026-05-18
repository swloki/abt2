//! H3Yun Sync gRPC Handler

use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_sync_service_server::AbtSyncService as GrpcSyncService, *,
};
use crate::handlers::{dt_to_string, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
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
    #[require_permission(Resource::Sync, Action::Write)]
    async fn sync_product(
        &self,
        request: Request<SyncProductRequest>,
    ) -> GrpcResult<SyncResponse> {
        let req = request.into_inner();
        if req.product_id <= 0 {
            return Err(error::validation("product_id", "产品 ID 无效"));
        }

        let state = AppState::get().await;
        let pool = state.pool();
        let client = abt::h3yun::get_h3yun_client();

        // 查询产品
        let product = abt::repositories::ProductRepo::find_by_id(&pool, req.product_id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::validation("product_id", "产品不存在"))?;

        // 查询分类路径
        let category_path =
            abt::h3yun::product_sync::fetch_category_path(&pool, req.product_id).await;

        // 直接执行同步
        match abt::h3yun::product_sync::sync_product(
            &pool,
            client,
            &product,
            category_path.as_ref(),
        )
        .await
        {
            Ok(()) => Ok(Response::new(SyncResponse {
                processed: 1,
                succeeded: 1,
                message: "同步成功".to_string(),
            })),
            Err(e) => Ok(Response::new(SyncResponse {
                processed: 1,
                succeeded: 0,
                message: format!("同步失败: {e}"),
            })),
        }
    }

    #[require_permission(Resource::Sync, Action::Write)]
    async fn sync_all_products(
        &self,
        _request: Request<SyncAllRequest>,
    ) -> GrpcResult<SyncResponse> {
        let state = AppState::get().await;
        let product_ids: Vec<i64> = {
            use abt::service::ProductService;
            let query = abt::models::ProductQuery {
                page: Some(1),
                page_size: Some(99999),
                ..Default::default()
            };
            state
                .product_service()
                .query(query)
                .await
                .map_err(error::err_to_status)?
                .0
                .into_iter()
                .map(|p| p.product_id)
                .collect()
        };

        let total = product_ids.len() as i32;
        abt::h3yun::set_batch_status(abt::h3yun::models::SyncBatchStatus::new(
            abt::h3yun::models::EntityType::Product,
            total,
        ));

        let sender = abt::h3yun::get_sync_event_sender();
        let mut queued = 0i32;

        for product_id in &product_ids {
            if sender
                .send(abt::h3yun::models::SyncEvent {
                    entity_type: abt::h3yun::models::EntityType::Product,
                    entity_id: *product_id,
                    is_batch: true,
                })
                .await
                .is_ok()
            {
                queued += 1;
            }
        }

        Ok(Response::new(SyncResponse {
            processed: queued,
            succeeded: 0,
            message: format!("{queued} sync events queued"),
        }))
    }

    #[require_permission(Resource::Sync, Action::Write)]
    async fn sync_all_inventory(
        &self,
        _request: Request<SyncAllRequest>,
    ) -> GrpcResult<SyncResponse> {
        let state = AppState::get().await;
        let pool = state.pool();

        let inventory_ids: Vec<i64> = sqlx::query_scalar!(
            r#"SELECT inventory_id FROM inventory WHERE quantity > 0 AND location_id IS NOT NULL"#
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| error::err_to_status(anyhow::anyhow!(e)))?;

        let total = inventory_ids.len() as i32;
        abt::h3yun::set_batch_status(abt::h3yun::models::SyncBatchStatus::new(
            abt::h3yun::models::EntityType::Inventory,
            total,
        ));

        let sender = abt::h3yun::get_sync_event_sender();
        let mut queued = 0i32;

        for inventory_id in &inventory_ids {
            if sender
                .send(abt::h3yun::models::SyncEvent {
                    entity_type: abt::h3yun::models::EntityType::Inventory,
                    entity_id: *inventory_id,
                    is_batch: true,
                })
                .await
                .is_ok()
            {
                queued += 1;
            }
        }

        Ok(Response::new(SyncResponse {
            processed: queued,
            succeeded: 0,
            message: format!("{queued} inventory sync events queued"),
        }))
    }

    #[require_permission(Resource::Sync, Action::Write)]
    async fn sync_inventory(
        &self,
        request: Request<SyncInventoryRequest>,
    ) -> GrpcResult<SyncResponse> {
        let req = request.into_inner();
        if req.product_id <= 0 {
            return Err(error::validation("product_id", "产品 ID 无效"));
        }

        let state = AppState::get().await;
        let pool = state.pool();
        let client = abt::h3yun::get_h3yun_client();

        let inventories: Vec<abt::models::InventoryDetail> = {
            use abt::service::InventoryService;
            state
                .inventory_service()
                .get_by_product(req.product_id)
                .await
                .map_err(error::err_to_status)?
        };

        let mut succeeded = 0i32;
        let mut failed = 0i32;
        let mut errors = Vec::new();

        for inv in &inventories {
            let data = abt::h3yun::inventory_sync::InventorySyncData {
                inventory_id: inv.inventory_id,
                product_id: inv.product_id,
                location_code: inv.location_code.clone(),
                warehouse_name: inv.warehouse_name.clone(),
                product_code: inv.product_code.clone(),
                product_name: inv.product_name.clone(),
                quantity: inv.quantity,
                unit: String::new(),
            };

            match abt::h3yun::inventory_sync::sync_inventory(&pool, client, &data).await {
                Ok(()) => succeeded += 1,
                Err(e) => {
                    failed += 1;
                    if errors.len() < 5 {
                        errors.push(format!("{}: {e}", data.location_code));
                    }
                }
            }
        }

        let message = if failed == 0 {
            format!("同步成功 {succeeded} 条库存")
        } else {
            let mut msg = format!("成功 {succeeded}，失败 {failed}");
            if !errors.is_empty() {
                msg.push_str(&format!(" — {}", errors.join("; ")));
            }
            msg
        };

        Ok(Response::new(SyncResponse {
            processed: inventories.len() as i32,
            succeeded,
            message,
        }))
    }

    #[require_permission(Resource::Sync, Action::Read)]
    async fn get_latest_sync_status(
        &self,
        request: Request<GetLatestSyncStatusRequest>,
    ) -> GrpcResult<SyncBatchStatus> {
        let req = request.into_inner();
        let batch_type = req.batch_type;
        if batch_type != "product" && batch_type != "inventory" {
            return Err(error::validation("batch_type", "必须是 product 或 inventory"));
        }

        let batch = abt::h3yun::get_batch_status(&batch_type);

        Ok(Response::new(SyncBatchStatus {
            batch_type,
            status: batch.as_ref().map(|b| b.status.clone()).unwrap_or_default(),
            total: batch.as_ref().map(|b| b.total).unwrap_or_default(),
            processed: batch.as_ref().map(|b| b.processed).unwrap_or_default(),
            succeeded: batch.as_ref().map(|b| b.succeeded).unwrap_or_default(),
            failed: batch.as_ref().map(|b| b.failed).unwrap_or_default(),
            started_at: dt_to_string(batch.as_ref().and_then(|b| b.started_at)),
            completed_at: dt_to_string(batch.as_ref().and_then(|b| b.completed_at)),
        }))
    }

    #[require_permission(Resource::Sync, Action::Read)]
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
        let client = abt::h3yun::get_h3yun_client();
        let schema_code = match entity_type {
            abt::h3yun::models::EntityType::Product => abt::h3yun::models::schema::PRODUCT,
            abt::h3yun::models::EntityType::Inventory => abt::h3yun::models::schema::WAREHOUSE,
        };

        let remote_items = client
            .query_list(schema_code)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("H3Yun query failed: {e}")))?;

        let local_mappings = abt::h3yun::sync_state::SyncStateRepo::find_all_by_type(
            &state.pool(),
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
