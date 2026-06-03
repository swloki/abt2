use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::{supplier_list, supplier_create, supplier_detail, supplier_edit};
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers")]
pub struct SupplierListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/table")]
pub struct SupplierTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/new")]
pub struct SupplierCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{id}")]
pub struct SupplierDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{id}/edit")]
pub struct SupplierEditPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{id}/delete")]
pub struct SupplierDeletePath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{id}/contacts")]
pub struct SupplierContactPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{sid}/contacts/{contact_id}")]
pub struct SupplierDeleteContactPath {
    pub sid: i64,
    pub contact_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{id}/bank-accounts")]
pub struct SupplierBankAccountPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/suppliers/{sid}/bank-accounts/{account_id}")]
pub struct SupplierDeleteBankAccountPath {
    pub sid: i64,
    pub account_id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            SupplierListPath::PATH,
            get(supplier_list::get_supplier_list),
        )
        .route(
            SupplierTablePath::PATH,
            get(supplier_list::get_supplier_table),
        )
        .route(
            SupplierCreatePath::PATH,
            get(supplier_create::get_supplier_create).post(supplier_create::post_supplier_create),
        )
        .route(
            SupplierDetailPath::PATH,
            get(supplier_detail::get_supplier_detail),
        )
        .route(
            SupplierEditPath::PATH,
            get(supplier_edit::get_supplier_edit).post(supplier_edit::post_supplier_edit),
        )
        .route(
            SupplierDeletePath::PATH,
            post(supplier_list::delete_supplier),
        )
        .route(
            SupplierContactPath::PATH,
            post(supplier_detail::create_supplier_contact),
        )
        .route(
            SupplierDeleteContactPath::PATH,
            post(supplier_detail::delete_supplier_contact),
        )
        .route(
            SupplierBankAccountPath::PATH,
            post(supplier_detail::create_supplier_bank_account),
        )
        .route(
            SupplierDeleteBankAccountPath::PATH,
            post(supplier_detail::delete_supplier_bank_account),
        )
}
