//! Inventory gRPC Handler

use crate::generated::abt::v1::{
    abt_inventory_service_server::AbtInventoryService as GrpcInventoryService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;
use common::error;
use tonic::{Request, Response};

// Import traits and types from abt
use abt::{
    InventoryCascadeService, InventoryLog as AbtInventoryLog, InventoryLogQuery as AbtInventoryLogQuery,
    InventoryQuery as AbtInventoryQuery, InventoryService, OperationType,
    SetSafetyStockRequest as AbtSetSafetyStockRequest, StockChangeRequest as AbtStockChangeRequest,
    StockTransferRequest as AbtStockTransferRequest,
};
use rust_decimal::Decimal;

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

/// Convert proto StockChangeRequest to abt StockChangeRequest
fn to_abt_stock_change_req(req: StockChangeRequest) -> AbtStockChangeRequest {
    AbtStockChangeRequest {
        product_id: req.product_id,
        location_id: req.location_id,
        quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
        operation_type: OperationType::In, // Will be overridden by specific methods
        ref_order_type: if req.ref_order_type.is_empty() {
            None
        } else {
            Some(req.ref_order_type)
        },
        ref_order_id: if req.ref_order_id.is_empty() {
            None
        } else {
            Some(req.ref_order_id)
        },
        operator: if req.operator.is_empty() {
            None
        } else {
            Some(req.operator)
        },
        remark: if req.remark.is_empty() {
            None
        } else {
            Some(req.remark)
        },
    }
}

/// Convert proto StockTransferRequest to abt StockTransferRequest
fn to_abt_transfer_req(req: StockTransferRequest) -> AbtStockTransferRequest {
    AbtStockTransferRequest {
        product_id: req.product_id,
        from_location_id: req.from_location_id,
        to_location_id: req.to_location_id,
        quantity: Decimal::from_f64_retain(req.quantity).unwrap_or(Decimal::ZERO),
        operator: if req.operator.is_empty() {
            None
        } else {
            Some(req.operator)
        },
        remark: if req.remark.is_empty() {
            None
        } else {
            Some(req.remark)
        },
    }
}

/// Convert abt InventoryLog to proto InventoryLogResponse
fn to_proto_log_response(log: AbtInventoryLog) -> InventoryLogResponse {
    InventoryLogResponse {
        log_id: log.log_id,
        inventory_id: log.inventory_id,
        product_id: log.product_id,
        location_id: log.location_id,
        change_qty: log.change_qty.to_string().parse().unwrap_or(0.0),
        before_qty: log.before_qty.to_string().parse().unwrap_or(0.0),
        after_qty: log.after_qty.to_string().parse().unwrap_or(0.0),
        operation_type: log.operation_type.to_string(),
        ref_order_type: log.ref_order_type.unwrap_or_default(),
        ref_order_id: log.ref_order_id.unwrap_or_default(),
        operator: log.operator.unwrap_or_default(),
        remark: log.remark.unwrap_or_default(),
        created_at: log.created_at.timestamp(),
    }
}

/// Convert proto SetSafetyStockRequest to abt type
fn to_abt_safety_stock_req(req: SetSafetyStockRequest) -> AbtSetSafetyStockRequest {
    AbtSetSafetyStockRequest {
        product_id: req.product_id,
        location_id: req.location_id,
        safety_stock: Decimal::from_f64_retain(req.safety_stock).unwrap_or(Decimal::ZERO),
    }
}

#[tonic::async_trait]
impl GrpcInventoryService for InventoryHandler {
    #[require_permission(Resource::Inventory, Action::Write)]
    async fn stock_in(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();
        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let mut abt_req = to_abt_stock_change_req(req);
        abt_req.operation_type = OperationType::In;

        let log = srv
            .stock_in(abt_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(to_proto_log_response(log)))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn stock_out(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();
        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let mut abt_req = to_abt_stock_change_req(req);
        abt_req.operation_type = OperationType::Out;

        let log = srv
            .stock_out(abt_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(to_proto_log_response(log)))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn adjust_stock(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();
        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let mut abt_req = to_abt_stock_change_req(req);
        abt_req.operation_type = OperationType::Adjust;

        let log = srv
            .adjust(abt_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(to_proto_log_response(log)))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn set_quantity(
        &self,
        request: Request<StockChangeRequest>,
    ) -> GrpcResult<InventoryLogResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();
        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let mut abt_req = to_abt_stock_change_req(req);
        abt_req.operation_type = OperationType::Adjust;

        let log = srv
            .set_quantity(abt_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(to_proto_log_response(log)))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn transfer_stock(
        &self,
        request: Request<StockTransferRequest>,
    ) -> GrpcResult<InventoryLogListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();
        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let abt_req = to_abt_transfer_req(req);

        let (out_log, in_log) = srv
            .transfer(abt_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(InventoryLogListResponse {
            items: vec![
                to_proto_log_response(out_log),
                to_proto_log_response(in_log),
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

        let query = AbtInventoryQuery {
            product_id: req.product_id,
            warehouse_id: req.warehouse_id,
            location_id: req.location_id,
            product_name: req.keyword.clone(),
            product_code: None,
            term_id: req.term_id,
            low_stock_only: None,
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
        };

        let result = srv
            .query(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryListResponse {
            items: result
                .items
                .into_iter()
                .map(|detail| InventoryDetailResponse {
                    inventory_id: detail.inventory_id,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    quantity: detail.quantity.to_string().parse().unwrap_or(0.0),
                    safety_stock: detail.safety_stock.to_string().parse().unwrap_or(0.0),
                })
                .collect(),
            total: result.total as u64,
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

        let items = srv
            .get_by_product(req.product_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryDetailListResponse {
            items: items
                .into_iter()
                .map(|detail| InventoryDetailResponse {
                    inventory_id: detail.inventory_id,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    quantity: detail.quantity.to_string().parse().unwrap_or(0.0),
                    safety_stock: detail.safety_stock.to_string().parse().unwrap_or(0.0),
                })
                .collect(),
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

        let items = srv
            .get_by_location(req.location_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryDetailListResponse {
            items: items
                .into_iter()
                .map(|detail| InventoryDetailResponse {
                    inventory_id: detail.inventory_id,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    quantity: detail.quantity.to_string().parse().unwrap_or(0.0),
                    safety_stock: detail.safety_stock.to_string().parse().unwrap_or(0.0),
                })
                .collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn get_low_stock_alert(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<InventoryDetailListResponse> {
        let state = AppState::get().await;
        let srv = state.inventory_service();

        let items = srv
            .list_low_stock()
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryDetailListResponse {
            items: items
                .into_iter()
                .map(|detail| InventoryDetailResponse {
                    inventory_id: detail.inventory_id,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    quantity: detail.quantity.to_string().parse().unwrap_or(0.0),
                    safety_stock: detail.safety_stock.to_string().parse().unwrap_or(0.0),
                })
                .collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Write)]
    async fn set_safety_stock(
        &self,
        request: Request<SetSafetyStockRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.inventory_service();
        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let abt_req = to_abt_safety_stock_req(req);

        srv.set_safety_stock(abt_req, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit()
            .await
            .map_err(error::sqlx_err_to_status)?;

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

        let query = AbtInventoryLogQuery {
            product_id: req.product_id,
            product_name: req.product_name,
            product_code: req.product_code,
            location_id: req.location_id,
            warehouse_id: req.warehouse_id,
            operation_type: req.operation_type.and_then(|s| s.parse().ok()),
            operator: None,
            start_date: req
                .start_time
                .map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_default()),
            end_date: req
                .end_time
                .map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap_or_default()),
            page: req.page.map(|p| p as i64),
            page_size: req.page_size.map(|p| p as i64),
        };

        let result = srv
            .query_logs(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryLogListResponse {
            items: result
                .items
                .into_iter()
                .map(|detail| InventoryLogResponse {
                    log_id: detail.log_id,
                    inventory_id: 0,
                    product_id: detail.product_id,
                    location_id: detail.location_id,
                    change_qty: detail.change_qty.to_string().parse().unwrap_or(0.0),
                    before_qty: detail.before_qty.to_string().parse().unwrap_or(0.0),
                    after_qty: detail.after_qty.to_string().parse().unwrap_or(0.0),
                    operation_type: detail.operation_type.to_string(),
                    ref_order_type: detail.ref_order_type.unwrap_or_default(),
                    ref_order_id: detail.ref_order_id.unwrap_or_default(),
                    operator: detail.operator.unwrap_or_default(),
                    remark: detail.remark.unwrap_or_default(),
                    created_at: detail.created_at.timestamp(),
                })
                .collect(),
            total: result.total as u64,
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

        let items = srv
            .list_logs_by_product(req.product_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryLogDetailListResponse {
            items: items
                .into_iter()
                .map(|detail| InventoryLogDetailResponse {
                    log_id: detail.log_id,
                    inventory_id: 0,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    change_qty: detail.change_qty.to_string().parse().unwrap_or(0.0),
                    before_qty: detail.before_qty.to_string().parse().unwrap_or(0.0),
                    after_qty: detail.after_qty.to_string().parse().unwrap_or(0.0),
                    operation_type: detail.operation_type.to_string(),
                    ref_order_type: detail.ref_order_type.unwrap_or_default(),
                    ref_order_id: detail.ref_order_id.unwrap_or_default(),
                    operator: detail.operator.unwrap_or_default(),
                    remark: detail.remark.unwrap_or_default(),
                    created_at: detail.created_at.timestamp(),
                })
                .collect(),
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

        let items = srv
            .list_logs_by_location(req.location_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryLogDetailListResponse {
            items: items
                .into_iter()
                .map(|detail| InventoryLogDetailResponse {
                    log_id: detail.log_id,
                    inventory_id: 0,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    change_qty: detail.change_qty.to_string().parse().unwrap_or(0.0),
                    before_qty: detail.before_qty.to_string().parse().unwrap_or(0.0),
                    after_qty: detail.after_qty.to_string().parse().unwrap_or(0.0),
                    operation_type: detail.operation_type.to_string(),
                    ref_order_type: detail.ref_order_type.unwrap_or_default(),
                    ref_order_id: detail.ref_order_id.unwrap_or_default(),
                    operator: detail.operator.unwrap_or_default(),
                    remark: detail.remark.unwrap_or_default(),
                    created_at: detail.created_at.timestamp(),
                })
                .collect(),
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

        let items = srv
            .list_logs_by_warehouse(req.warehouse_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(InventoryLogDetailListResponse {
            items: items
                .into_iter()
                .map(|detail| InventoryLogDetailResponse {
                    log_id: detail.log_id,
                    inventory_id: 0,
                    product_id: detail.product_id,
                    product_name: detail.product_name,
                    product_code: detail.product_code,
                    location_id: detail.location_id,
                    location_name: detail.location_code,
                    warehouse_id: 0,
                    warehouse_name: detail.warehouse_name,
                    change_qty: detail.change_qty.to_string().parse().unwrap_or(0.0),
                    before_qty: detail.before_qty.to_string().parse().unwrap_or(0.0),
                    after_qty: detail.after_qty.to_string().parse().unwrap_or(0.0),
                    operation_type: detail.operation_type.to_string(),
                    ref_order_type: detail.ref_order_type.unwrap_or_default(),
                    ref_order_id: detail.ref_order_id.unwrap_or_default(),
                    operator: detail.operator.unwrap_or_default(),
                    remark: detail.remark.unwrap_or_default(),
                    created_at: detail.created_at.timestamp(),
                })
                .collect(),
        }))
    }

    #[require_permission(Resource::Inventory, Action::Read)]
    async fn cascade_inventory(
        &self,
        request: Request<CascadeInventoryRequest>,
    ) -> GrpcResult<CascadeInventoryResponse> {
        let req = request.into_inner();

        // 输入校验
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
        let srv = state.inventory_cascade_service();

        let result = srv
            .cascade_inventory(product_id, product_code, max_results)
            .await
            .map_err(common::error::err_to_status)?;

        Ok(Response::new(CascadeInventoryResponse {
            product_id: result.product_id,
            product_code: result.product_code,
            product_name: result.product_name,
            bom_groups: result
                .bom_groups
                .into_iter()
                .map(|g| BomCascadeGroup {
                    bom_id: g.bom_id,
                    bom_name: g.bom_name,
                    children: g
                        .children
                        .into_iter()
                        .map(|c| ChildNodeInventory {
                            node_id: c.node_id,
                            product_id: c.product_id,
                            product_code: c.product_code,
                            product_name: c.product_name,
                            unit: c.unit.unwrap_or_default(),
                            quantity: c.quantity.to_string().parse().unwrap_or(0.0),
                            total_stock: c.total_stock.to_string().parse().unwrap_or(0.0),
                            loss_rate: c.loss_rate.to_string().parse().unwrap_or(0.0),
                            order: c.order,
                            parent_node_id: c.parent_node_id,
                        })
                        .collect(),
                })
                .collect(),
        }))
    }
}
