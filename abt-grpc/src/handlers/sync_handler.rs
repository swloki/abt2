//! H3Yun Sync gRPC Handler

use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_sync_service_server::AbtSyncService as GrpcSyncService, *,
};
use crate::handlers::GrpcResult;
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
        let pool = state.core_pool();
        let client = abt_core::h3yun::H3YunClient::new();

        // 查询产品 (abt_v2)
        let product = {
            use abt_core::master_data::product::repo::ProductRepo;
            let mut conn = pool.acquire().await.map_err(error::sqlx_err_to_status)?;
            ProductRepo.find_by_id(&mut conn, req.product_id)
                .await
                .map_err(|e| error::err_to_status(e.into()))?
                .ok_or_else(|| error::validation("product_id", "产品不存在"))?
        };

        // 查询分类路径
        let category_path =
            abt_core::h3yun::product_sync::fetch_category_path(&pool, req.product_id).await;

        // 直接执行同步
        match abt_core::h3yun::product_sync::sync_product(
            &pool,
            &client,
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
        use abt_core::shared::event_bus::model::EventPublishRequest;
        use abt_core::shared::enums::event::DomainEventType;
        use abt_core::shared::types::ServiceContext;

        let state = AppState::get().await;

        // 查询所有未同步的产品
        let product_ids = abt_core::h3yun::sync_state::SyncStateRepo::find_entity_ids_never_synced(
            &state.core_pool(),
            abt_core::h3yun::models::EntityType::Product,
            500,
        )
        .await
        .map_err(error::err_to_status)?;

        let total = product_ids.len() as i32;
        let mut queued = 0i32;
        let pool = state.core_pool();

        for product_id in &product_ids {
            let mut conn = pool.acquire().await.map_err(error::sqlx_err_to_status)?;
            let ctx = ServiceContext::system(&mut conn);
            if state.event_bus().publish(ctx, EventPublishRequest {
                event_type: DomainEventType::ProductCreated,
                aggregate_type: "Product".to_string(),
                aggregate_id: *product_id,
                payload: serde_json::json!({}),
                idempotency_key: None,
            }).await.is_ok() {
                queued += 1;
            }
        }

        Ok(Response::new(SyncResponse {
            processed: total,
            succeeded: queued,
            message: format!("{queued} sync events queued"),
        }))
    }

    #[require_permission(Resource::Sync, Action::Write)]
    async fn sync_all_inventory(
        &self,
        _request: Request<SyncAllRequest>,
    ) -> GrpcResult<SyncResponse> {
        use abt_core::shared::event_bus::model::EventPublishRequest;
        use abt_core::shared::enums::event::DomainEventType;
        use abt_core::shared::types::ServiceContext;

        let state = AppState::get().await;
        let core_pool = state.core_pool();

        let stock_ledger_ids: Vec<i64> = sqlx::query_scalar(
            r#"SELECT id FROM stock_ledger WHERE quantity > 0"#,
        )
        .fetch_all(&core_pool)
        .await
        .map_err(|e| error::err_to_status(anyhow::anyhow!(e)))?;

        let total = stock_ledger_ids.len() as i32;
        let mut queued = 0i32;

        for ledger_id in &stock_ledger_ids {
            let mut conn = core_pool.acquire().await.map_err(error::sqlx_err_to_status)?;
            let ctx = ServiceContext::system(&mut conn);
            if state.event_bus().publish(ctx, EventPublishRequest {
                event_type: DomainEventType::H3YunInventorySync,
                aggregate_type: "inventory".to_string(),
                aggregate_id: *ledger_id,
                payload: serde_json::json!({}),
                idempotency_key: None,
            }).await.is_ok() {
                queued += 1;
            }
        }

        Ok(Response::new(SyncResponse {
            processed: total,
            succeeded: queued,
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
        let core_pool = state.core_pool();
        let client = abt_core::h3yun::H3YunClient::new();

        let inventories = {
            use abt_core::wms::inventory::InventoryService;
            let srv = state.inventory_service();
            let mut tx = state.begin_core_transaction().await
                .map_err(error::err_to_status)?;
            let ctx = abt_core::shared::types::ServiceContext::system(&mut tx);
            srv.get_by_product(ctx, req.product_id).await
                .map_err(crate::handlers::domain_to_status)?
        };

        let mut succeeded = 0i32;
        let mut failed = 0i32;
        let mut errors = Vec::new();

        for inv in &inventories {
            let data = abt_core::h3yun::inventory_sync::InventorySyncData {
                inventory_id: inv.stock_ledger_id,
                product_id: inv.product_id,
                location_code: inv.bin_code.clone(),
                warehouse_name: inv.warehouse_name.clone(),
                product_code: inv.product_code.clone(),
                product_name: inv.product_name.clone(),
                quantity: inv.quantity,
                unit: String::new(),
            };

            match abt_core::h3yun::inventory_sync::sync_inventory(&core_pool, &client, &data).await {
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

        // Batch status no longer tracked in memory — return empty status
        Ok(Response::new(SyncBatchStatus {
            batch_type,
            status: String::new(),
            total: 0,
            processed: 0,
            succeeded: 0,
            failed: 0,
            started_at: String::new(),
            completed_at: String::new(),
        }))
    }

    #[require_permission(Resource::Sync, Action::Read)]
    async fn reconcile(
        &self,
        request: Request<ReconcileRequest>,
    ) -> GrpcResult<ReconcileResponse> {
        let req = request.into_inner();
        let entity_type = match req.entity_type.as_str() {
            "product" => abt_core::h3yun::models::EntityType::Product,
            "inventory" => abt_core::h3yun::models::EntityType::Inventory,
            _ => return Err(error::validation("entity_type", "必须是 product 或 inventory")),
        };

        let state = AppState::get().await;
        let client = abt_core::h3yun::H3YunClient::new();
        let schema_code = match entity_type {
            abt_core::h3yun::models::EntityType::Product => abt_core::h3yun::models::schema::PRODUCT,
            abt_core::h3yun::models::EntityType::Inventory => abt_core::h3yun::models::schema::WAREHOUSE,
        };

        let remote_items = client
            .query_list(schema_code)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("H3Yun query failed: {e}")))?;

        let local_mappings = abt_core::h3yun::sync_state::SyncStateRepo::find_all_by_type(
            &state.core_pool(),
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
