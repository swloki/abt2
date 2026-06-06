use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{wms_warehouse_list, wms_warehouse_create, wms_warehouse_detail};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses")]
pub struct WarehouseListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/table")]
pub struct WarehouseTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/create")]
pub struct WarehouseCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/{id}")]
pub struct WarehouseDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/{id}/edit")]
pub struct WarehouseEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/{id}/delete")]
pub struct WarehouseDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/{id}/zones")]
pub struct WarehouseZoneCreatePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/zones/{zone_id}")]
pub struct WarehouseZonePath {
    pub zone_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/wms/warehouses/zones/{zone_id}/bins")]
pub struct WarehouseZoneBinsPath {
    pub zone_id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(WarehouseListPath::PATH, get(wms_warehouse_list::get_warehouse_list))
        .route(WarehouseTablePath::PATH, get(wms_warehouse_list::get_warehouse_table))
        .route(WarehouseCreatePath::PATH, get(wms_warehouse_create::get_warehouse_create).post(wms_warehouse_create::create_warehouse))
        .route(WarehouseDetailPath::PATH, get(wms_warehouse_detail::get_warehouse_detail))
        .route(WarehouseEditPath::PATH, get(wms_warehouse_detail::get_warehouse_edit).post(wms_warehouse_detail::update_warehouse))
        .route(WarehouseDeletePath::PATH, post(wms_warehouse_detail::delete_warehouse))
        .route(WarehouseZoneCreatePath::PATH, get(wms_warehouse_detail::get_zones).post(wms_warehouse_detail::create_zone))
        .route(WarehouseZonePath::PATH, get(wms_warehouse_detail::get_zone_edit_form).put(wms_warehouse_detail::update_zone).delete(wms_warehouse_detail::delete_zone))
        .route(WarehouseZoneBinsPath::PATH, get(wms_warehouse_detail::get_zone_bins))
}
