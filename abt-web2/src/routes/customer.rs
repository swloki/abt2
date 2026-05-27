use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::customer_list;
use crate::pages::customer_detail;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers")]
pub struct CustomerListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/new")]
pub struct CreateCustomerPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/table")]
pub struct CustomerTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}/edit-form")]
pub struct EditCustomerFormPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}")]
pub struct UpdateCustomerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}/delete")]
pub struct DeleteCustomerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}")]
pub struct CustomerDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}/contacts")]
pub struct CreateContactPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{cid}/contacts/{contact_id}")]
pub struct DeleteContactPath {
    pub cid: i64,
    pub contact_id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}/addresses")]
pub struct CreateAddressPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{cid}/addresses/{address_id}")]
pub struct DeleteAddressPath {
    pub cid: i64,
    pub address_id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(CustomerListPath::PATH, get(customer_list::get_customer_list))
        .route(CustomerTablePath::PATH, get(customer_list::get_customer_table))
        .route(CreateCustomerPath::PATH, post(customer_list::create_customer))
        .route(EditCustomerFormPath::PATH, get(customer_list::get_edit_customer_form))
        .route(CustomerDetailPath::PATH, get(customer_detail::get_customer_detail))
        .route(CreateContactPath::PATH, post(customer_detail::create_contact))
        .route(DeleteContactPath::PATH, post(customer_detail::delete_contact))
        .route(CreateAddressPath::PATH, post(customer_detail::create_address))
        .route(DeleteAddressPath::PATH, post(customer_detail::delete_address))
        .route(UpdateCustomerPath::PATH, post(customer_list::update_customer))
        .route(DeleteCustomerPath::PATH, post(customer_list::delete_customer))
}
