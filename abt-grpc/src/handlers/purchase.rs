use common::error;
use rust_decimal::Decimal;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    purchase_service_server::PurchaseService as GrpcPurchaseService,
    *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

use abt::{PurchaseOrderService, SupplierPriceService};

pub struct PurchaseHandler;

impl PurchaseHandler {
    pub fn new() -> Self { Self }
}

impl Default for PurchaseHandler {
    fn default() -> Self { Self::new() }
}

fn po_status_to_proto(status: i16) -> i32 {
    match status {
        1 => PurchaseOrderStatus::Draft as i32,
        2 => PurchaseOrderStatus::Submitted as i32,
        3 => PurchaseOrderStatus::Approved as i32,
        4 => PurchaseOrderStatus::PartialReceived as i32,
        5 => PurchaseOrderStatus::FullyReceived as i32,
        6 => PurchaseOrderStatus::Reconciled as i32,
        7 => PurchaseOrderStatus::Closed as i32,
        _ => PurchaseOrderStatus::Unspecified as i32,
    }
}

fn proto_to_po_status(status: i32) -> i16 {
    match PurchaseOrderStatus::try_from(status).unwrap_or(PurchaseOrderStatus::Unspecified) {
        PurchaseOrderStatus::Draft => 1,
        PurchaseOrderStatus::Submitted => 2,
        PurchaseOrderStatus::Approved => 3,
        PurchaseOrderStatus::PartialReceived => 4,
        PurchaseOrderStatus::FullyReceived => 5,
        PurchaseOrderStatus::Reconciled => 6,
        PurchaseOrderStatus::Closed => 7,
        _ => 0,
    }
}

fn po_type_to_proto(order_type: i16) -> i32 {
    match order_type {
        1 => PurchaseOrderType::Production as i32,
        2 => PurchaseOrderType::Miscellaneous as i32,
        _ => PurchaseOrderType::Unspecified as i32,
    }
}

fn proto_to_po_type(order_type: i32) -> i16 {
    match PurchaseOrderType::try_from(order_type).unwrap_or(PurchaseOrderType::Unspecified) {
        PurchaseOrderType::Production => 1,
        PurchaseOrderType::Miscellaneous => 2,
        _ => 0,
    }
}

fn parse_po_items(items: &[CreatePurchaseOrderItem]) -> Result<Vec<abt::models::PurchaseOrderItemInput>, tonic::Status> {
    items.iter().map(|item| {
        let unit_price: Decimal = item.unit_price.parse()
            .map_err(|_| error::validation("unit_price", "Invalid decimal format"))?;
        let quantity: Decimal = item.quantity.parse()
            .map_err(|_| error::validation("quantity", "Invalid decimal format"))?;
        Ok(abt::models::PurchaseOrderItemInput {
            product_id: item.product_id,
            unit_price,
            quantity,
            remark: empty_to_none(item.remark.clone()),
        })
    }).collect()
}

fn po_detail_to_proto(d: &abt::models::PurchaseOrderDetail) -> PurchaseOrder {
    PurchaseOrder {
        po_id: d.po_id,
        po_no: d.po_no.clone(),
        supplier_id: d.supplier_id,
        supplier_name: d.supplier_name.clone().unwrap_or_default(),
        order_type: po_type_to_proto(d.order_type),
        status: po_status_to_proto(d.status),
        total_amount: d.total_amount.to_string(),
        remark: d.remark.clone().unwrap_or_default(),
        operator_id: d.operator_id.unwrap_or(0),
        created_at: d.created_at.timestamp(),
        updated_at: d.updated_at.timestamp(),
        items: vec![],
    }
}

fn po_with_items_to_proto(wi: &abt::models::PurchaseOrderWithItems) -> PurchaseOrder {
    PurchaseOrder {
        po_id: wi.order.po_id,
        po_no: wi.order.po_no.clone(),
        supplier_id: wi.order.supplier_id,
        supplier_name: String::new(),
        order_type: po_type_to_proto(wi.order.order_type),
        status: po_status_to_proto(wi.order.status),
        total_amount: wi.order.total_amount.to_string(),
        remark: wi.order.remark.clone().unwrap_or_default(),
        operator_id: wi.order.operator_id.unwrap_or(0),
        created_at: wi.order.created_at.timestamp(),
        updated_at: wi.order.updated_at.timestamp(),
        items: wi.items.iter().map(|i| PurchaseOrderItem {
            item_id: i.item_id,
            po_id: i.po_id,
            product_id: i.product_id,
            product_code: i.product_code.clone().unwrap_or_default(),
            product_name: i.product_name.clone().unwrap_or_default(),
            unit: i.unit.clone().unwrap_or_default(),
            unit_price: i.unit_price.to_string(),
            quantity: i.quantity.to_string(),
            received_qty: i.received_qty.to_string(),
            subtotal: i.subtotal.to_string(),
            remark: i.remark.clone().unwrap_or_default(),
        }).collect(),
    }
}

#[tonic::async_trait]
impl GrpcPurchaseService for PurchaseHandler {
    #[require_permission(Resource::Purchase, Action::Write)]
    async fn upsert_supplier_price(&self, request: Request<UpsertSupplierPriceRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_price_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let unit_price: Decimal = req.unit_price.parse()
            .map_err(|_| error::validation("unit_price", "Invalid decimal format"))?;

        let valid_from = chrono::DateTime::from_timestamp(req.valid_from, 0)
            .ok_or_else(|| error::validation("valid_from", "Invalid timestamp"))?;
        let valid_until = chrono::DateTime::from_timestamp(req.valid_until, 0)
            .ok_or_else(|| error::validation("valid_until", "Invalid timestamp"))?;

        let id = srv.upsert(
            req.supplier_id,
            req.product_id,
            unit_price,
            valid_from,
            valid_until,
            Some(auth.user_id),
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Purchase, Action::Read)]
    async fn list_supplier_prices(&self, request: Request<ListSupplierPricesRequest>) -> GrpcResult<SupplierPriceListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_price_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::models::SupplierPriceQuery {
            supplier_id: req.supplier_id,
            product_id: req.product_id,
            active_only: req.active_only,
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(SupplierPriceListResponse {
            items: result.items.into_iter().map(|sp| SupplierPrice {
                price_id: sp.price_id,
                supplier_id: sp.supplier_id,
                product_id: sp.product_id,
                product_code: sp.product_code.unwrap_or_default(),
                product_name: sp.product_name.unwrap_or_default(),
                unit: sp.unit.unwrap_or_default(),
                unit_price: sp.unit_price.to_string(),
                valid_from: sp.valid_from.timestamp(),
                valid_until: sp.valid_until.timestamp(),
                operator_id: sp.operator_id.unwrap_or(0),
                created_at: sp.created_at.timestamp(),
            }).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    #[require_permission(Resource::Purchase, Action::Write)]
    async fn create_purchase_order(&self, request: Request<CreatePurchaseOrderRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.purchase_order_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items = parse_po_items(&req.items)?;

        let id = srv.create(
            req.supplier_id,
            proto_to_po_type(req.order_type),
            empty_to_none(req.remark),
            Some(auth.user_id),
            items,
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Purchase, Action::Write)]
    async fn update_purchase_order(&self, request: Request<UpdatePurchaseOrderRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.purchase_order_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items = parse_po_items(&req.items)?;

        srv.update(
            req.po_id,
            req.supplier_id,
            empty_to_none(req.remark),
            items,
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Purchase, Action::Delete)]
    async fn delete_purchase_order(&self, request: Request<DeleteRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.purchase_order_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        srv.delete(req.id, &mut tx).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Purchase, Action::Read)]
    async fn get_purchase_order(&self, request: Request<GetPurchaseOrderRequest>) -> GrpcResult<PurchaseOrderResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.purchase_order_service();

        let wi = srv.get_by_id(req.po_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("PurchaseOrder", &req.po_id.to_string()))?;

        Ok(Response::new(PurchaseOrderResponse {
            purchase_order: Some(po_with_items_to_proto(&wi)),
        }))
    }

    #[require_permission(Resource::Purchase, Action::Read)]
    async fn list_purchase_orders(&self, request: Request<ListPurchaseOrdersRequest>) -> GrpcResult<PurchaseOrderListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.purchase_order_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::models::PurchaseOrderQuery {
            keyword: req.keyword,
            supplier_id: req.supplier_id,
            order_type: req.order_type.map(proto_to_po_type),
            status: req.status.map(proto_to_po_status),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(PurchaseOrderListResponse {
            items: result.items.into_iter().map(|d| po_detail_to_proto(&d)).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    #[require_permission(Resource::Purchase, Action::Write)]
    async fn update_purchase_order_status(&self, request: Request<UpdatePurchaseOrderStatusRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.purchase_order_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let status = proto_to_po_status(req.status);
        srv.update_status(req.po_id, status, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
