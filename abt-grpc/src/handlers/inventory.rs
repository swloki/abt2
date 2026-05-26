//! Inventory gRPC Handler — 委托给 abt-core InventoryService

use crate::generated::abt::v1::{
    abt_inventory_service_server::AbtInventoryService as GrpcInventoryService, *,
};
use crate::handlers::GrpcResult;
use crate::handlers::domain_to_status;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;
use common::error;
use tonic::{Request, Response, Status};

use abt_core::wms::inventory::{
    InventoryQueryFilter, InventoryService, StockChangeReq, StockTransferReq,
    TransactionLogFilter,
};
use abt_core::wms::inventory::repo::InventoryRepo;
use abt_core::shared::types::ServiceContext;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

pub struct InventoryHandler;

impl InventoryHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InventoryHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析 proto location_id → abt-core (warehouse_id, zone_id, bin_id)
async fn resolve_bin_location(
    location_id: i64,
    state: &AppState,
) -> Result<(i64, i64, i64), Status> {
    let pool = state.core_pool();
    let mut conn = pool.acquire().await.map_err(error::sqlx_err_to_status)?;

    InventoryRepo::resolve_bin(&mut conn, location_id)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::not_found(format!("Location#{location_id} not found")))
}

fn to_proto_log_response(result: &abt_core::wms::inventory::StockOperationResult, operation_type: &str) -> InventoryLogResponse {
    InventoryLogResponse {
        log_id: result.transaction_id,
        inventory_id: result.stock_ledger_id,
        product_id: result.product_id,
        location_id: result.bin_id,
        change_qty: result.change_qty.to_f64().unwrap_or(0.0),
        before_qty: result.before_qty.to_f64().unwrap_or(0.0),
        after_qty: result.after_qty.to_f64().unwrap_or(0.0),
        operation_type: operation_type.to_string(),
        ref_order_type: String::new(),
        ref_order_id: String::new(),
        operator: String::new(),
        remark: String::new(),
        created_at: 0,
    }
}

fn detail_to_proto(d: abt_core::wms::inventory::InventoryDetailView) -> InventoryDetailResponse {
    InventoryDetailResponse {
        inventory_id: d.stock_ledger_id,
        product_id: d.product_id,
        product_name: d.product_name,
        product_code: d.product_code,
        location_id: d.bin_id,
        location_name: d.bin_code,
        warehouse_id: d.warehouse_id,
        warehouse_name: d.warehouse_name,
        quantity: d.quantity.to_f64().unwrap_or(0.0),
        safety_stock: d.safety_stock.to_f64().unwrap_or(0.0),
    }
}

fn txn_detail_to_proto(d: abt_core::wms::inventory::TransactionDetailView) -> InventoryLogDetailResponse {
    InventoryLogDetailResponse {
        log_id: d.id,
        inventory_id: 0,
        product_id: d.product_id,
        product_name: d.product_name,
        product_code: d.product_code,
        location_id: d.bin_id,
        location_name: d.bin_code,
        warehouse_id: d.warehouse_id,
        warehouse_name: d.warehouse_name,
        change_qty: d.quantity.to_f64().unwrap_or(0.0),
        before_qty: 0.0,
        after_qty: 0.0,
        operation_type: format!("{:?}", d.transaction_type),
        ref_order_type: d.source_type,
        ref_order_id: d.source_id.to_string(),
        operator: d.operator_id.to_string(),
        remark: d.remark.unwrap_or_default(),
        created_at: d.created_at.timestamp(),
    }
}

#[tonic::async_trait]
impl GrpcInventoryService for InventoryHandler {
    #[require_permission(Resource::Inventory, Action::Write)]
    async fn stock_in(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let (wh_id, zone_id, bin_id) = resolve_bin_location(req.location_id, &state).await?;

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let result = srv.stock_in(ctx, StockChangeReq {
            product_id: req.product_id,
            warehouse_id: wh_id,
            zone_id,
            bin_id,
            quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
            ref_order_type: empty_to_none(req.ref_order_type),
            ref_order_id: empty_to_none(req.ref_order_id),
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        send_h3yun_sync(result.stock_ledger_id).await?;
        Ok(Response::new(to_proto_log_response(&result, "in")))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn stock_out(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let (wh_id, zone_id, bin_id) = resolve_bin_location(req.location_id, &state).await?;

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let result = srv.stock_out(ctx, StockChangeReq {
            product_id: req.product_id,
            warehouse_id: wh_id,
            zone_id,
            bin_id,
            quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
            ref_order_type: empty_to_none(req.ref_order_type),
            ref_order_id: empty_to_none(req.ref_order_id),
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        send_h3yun_sync(result.stock_ledger_id).await?;
        Ok(Response::new(to_proto_log_response(&result, "out")))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn adjust_stock(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let (wh_id, zone_id, bin_id) = resolve_bin_location(req.location_id, &state).await?;

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let result = srv.adjust(ctx, StockChangeReq {
            product_id: req.product_id,
            warehouse_id: wh_id,
            zone_id,
            bin_id,
            quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
            ref_order_type: empty_to_none(req.ref_order_type),
            ref_order_id: empty_to_none(req.ref_order_id),
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        send_h3yun_sync(result.stock_ledger_id).await?;
        Ok(Response::new(to_proto_log_response(&result, "adjust")))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn set_quantity(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let (wh_id, zone_id, bin_id) = resolve_bin_location(req.location_id, &state).await?;

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let result = srv.set_quantity(ctx, StockChangeReq {
            product_id: req.product_id,
            warehouse_id: wh_id,
            zone_id,
            bin_id,
            quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
            ref_order_type: empty_to_none(req.ref_order_type),
            ref_order_id: empty_to_none(req.ref_order_id),
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        send_h3yun_sync(result.stock_ledger_id).await?;
        Ok(Response::new(to_proto_log_response(&result, "adjust")))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn transfer_stock(
        &self,
        request: Request<StockTransferRequest>,
    ) -> GrpcResult<InventoryLogListResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let (from_wh, from_zone, _) = resolve_bin_location(req.from_location_id, &state).await?;
        let (to_wh, to_zone, _) = resolve_bin_location(req.to_location_id, &state).await?;

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let (out_result, in_result) = srv.transfer(ctx, StockTransferReq {
            product_id: req.product_id,
            from_warehouse_id: from_wh,
            from_zone_id: from_zone,
            from_bin_id: req.from_location_id,
            to_warehouse_id: to_wh,
            to_zone_id: to_zone,
            to_bin_id: req.to_location_id,
            quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        send_h3yun_sync(out_result.stock_ledger_id).await?;
        send_h3yun_sync(in_result.stock_ledger_id).await?;

        Ok(Response::new(InventoryLogListResponse {
            items: vec![
                to_proto_log_response(&out_result, "transfer"),
                to_proto_log_response(&in_result, "transfer"),
            ],
            total: 2,
            page: 1,
            page_size: 2,
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn query_inventory(
        &self,
        request: Request<InventoryQueryRequest>,
    ) -> GrpcResult<InventoryListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let filter = InventoryQueryFilter {
            product_id: req.product_id,
            keyword: req.keyword,
            warehouse_id: req.warehouse_id,
            bin_id: req.location_id,
        };

        let result = srv.query(ctx, filter, req.page.unwrap_or(1), req.page_size.unwrap_or(20))
            .await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryListResponse {
            items: result.items.into_iter().map(detail_to_proto).collect(),
            total: result.total,
            page: result.page,
            page_size: result.page_size,
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_inventory_by_product(
        &self,
        request: Request<GetInventoryByProductRequest>,
    ) -> GrpcResult<InventoryDetailListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let items = srv.get_by_product(ctx, req.product_id).await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryDetailListResponse {
            items: items.into_iter().map(detail_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_inventory_by_location(
        &self,
        request: Request<GetInventoryByLocationRequest>,
    ) -> GrpcResult<InventoryDetailListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let items = srv.get_by_bin(ctx, req.location_id).await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryDetailListResponse {
            items: items.into_iter().map(detail_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_low_stock_alert(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<InventoryDetailListResponse> {
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let items = srv.list_low_stock(ctx).await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryDetailListResponse {
            items: items.into_iter().map(detail_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn set_safety_stock(
        &self,
        request: Request<SetSafetyStockRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        srv.set_safety_stock(
            ctx,
            req.product_id,
            req.location_id,
            Decimal::from_f64_retain(req.safety_stock).unwrap_or(Decimal::ZERO),
        ).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn query_inventory_logs(
        &self,
        request: Request<InventoryLogQueryRequest>,
    ) -> GrpcResult<InventoryLogListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let filter = TransactionLogFilter {
            product_id: req.product_id,
            product_name: req.product_name,
            product_code: req.product_code,
            bin_id: req.location_id,
            warehouse_id: req.warehouse_id,
            transaction_type: req.operation_type,
            start_date: req.start_time.map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_default()),
            end_date: req.end_time.map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_default()),
        };

        let result = srv.query_logs(ctx, filter, req.page.unwrap_or(1), req.page_size.unwrap_or(20))
            .await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryLogListResponse {
            items: result.items.into_iter().map(|d| InventoryLogResponse {
                log_id: d.id,
                inventory_id: 0,
                product_id: d.product_id,
                location_id: d.bin_id,
                change_qty: d.quantity.to_f64().unwrap_or(0.0),
                before_qty: 0.0,
                after_qty: 0.0,
                operation_type: format!("{:?}", d.transaction_type),
                ref_order_type: d.source_type,
                ref_order_id: d.source_id.to_string(),
                operator: d.operator_id.to_string(),
                remark: d.remark.unwrap_or_default(),
                created_at: d.created_at.timestamp(),
            }).collect(),
            total: result.total,
            page: result.page,
            page_size: result.page_size,
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_logs_by_product(
        &self,
        request: Request<GetLogsByProductRequest>,
    ) -> GrpcResult<InventoryLogDetailListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let items = srv.list_logs_by_product(ctx, req.product_id).await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryLogDetailListResponse {
            items: items.into_iter().map(txn_detail_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_logs_by_location(
        &self,
        request: Request<GetLogsByLocationRequest>,
    ) -> GrpcResult<InventoryLogDetailListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let items = srv.list_logs_by_bin(ctx, req.location_id).await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryLogDetailListResponse {
            items: items.into_iter().map(txn_detail_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_logs_by_warehouse(
        &self,
        request: Request<GetLogsByWarehouseRequest>,
    ) -> GrpcResult<InventoryLogDetailListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let items = srv.list_logs_by_warehouse(ctx, req.warehouse_id).await.map_err(domain_to_status)?;

        Ok(Response::new(InventoryLogDetailListResponse {
            items: items.into_iter().map(txn_detail_to_proto).collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn cascade_inventory(
        &self,
        request: Request<CascadeInventoryRequest>,
    ) -> GrpcResult<CascadeInventoryResponse> {
        let req = request.into_inner();

        let (product_id, product_code) = match req.product_identifier {
            Some(cascade_inventory_request::ProductIdentifier::ProductId(id)) => (Some(id), None),
            Some(cascade_inventory_request::ProductIdentifier::ProductCode(code)) => (None, Some(code)),
            None => (None, None),
        };

        if product_id.is_none() && product_code.is_none() {
            return Err(common::error::validation(
                "product_identifier",
                "必须提供 product_id 或 product_code",
            ));
        }

        let max_results = req.max_results.unwrap_or(500);
        let state = AppState::get().await;

        let mut tx = state.begin_core_transaction().await.map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let query = abt_core::wms::inventory_cascade::CascadeInventoryQuery {
            product_id,
            product_code,
            max_results,
        };

        let srv = abt_core::wms::inventory_cascade::implt::InventoryCascadeServiceImpl::new();
        let result = abt_core::wms::inventory_cascade::InventoryCascadeService::cascade_inventory(&srv, ctx, query)
            .await.map_err(domain_to_status)?;

        Ok(Response::new(CascadeInventoryResponse {
            product_id: result.product_id,
            product_code: result.product_code,
            product_name: result.product_name,
            bom_groups: result.bom_groups.into_iter().map(|g| BomCascadeGroup {
                bom_id: g.bom_id,
                bom_name: g.bom_name,
                children: g.children.into_iter().map(|c| ChildNodeInventory {
                    node_id: c.node_id,
                    product_id: c.product_id,
                    product_code: c.product_code,
                    product_name: c.product_name,
                    unit: c.unit.unwrap_or_default(),
                    quantity: c.quantity.to_f64().unwrap_or(0.0),
                    total_stock: c.total_stock.to_f64().unwrap_or(0.0),
                    loss_rate: c.loss_rate.to_f64().unwrap_or(0.0),
                    order: c.order,
                    parent_node_id: c.parent_node_id,
                }).collect(),
            }).collect(),
        }))
    }
}

fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

async fn send_h3yun_sync(entity_id: i64) -> Result<(), Status> {
    use abt_core::shared::event_bus::model::EventPublishRequest;
    use abt_core::shared::enums::event::DomainEventType;

    let state = AppState::get().await;
    let pool = state.core_pool();
    let mut conn = pool.acquire().await.map_err(|e| Status::internal(e.to_string()))?;
    let ctx = ServiceContext::system(&mut conn);
    state.event_bus().publish(ctx, EventPublishRequest {
        event_type: DomainEventType::H3YunInventorySync,
        aggregate_type: "inventory".to_string(),
        aggregate_id: entity_id,
        payload: serde_json::json!({}),
        idempotency_key: None,
    }).await.map_err(|e| Status::internal(e.to_string()))?;
    Ok(())
}
