use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::customer_list;
use crate::pages::customer_detail;
use crate::pages::customer_create;
use crate::pages::customer_edit;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers")]
pub struct CustomerListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/new")]
pub struct CreateCustomerPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}/edit")]
pub struct EditCustomerPath {
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

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/customers/{id}/transactions")]
pub struct CustomerTransactionsPath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(CustomerListPath::PATH, get(customer_list::get_customer_list))
.route(CreateCustomerPath::PATH, get(customer_create::get_customer_create).post(customer_create::post_customer_create))
        .route(EditCustomerPath::PATH, get(customer_edit::get_customer_edit).post(customer_edit::post_customer_edit))
        .route(CustomerDetailPath::PATH, get(customer_detail::get_customer_detail))
        .route(CreateContactPath::PATH, post(customer_detail::create_contact))
        .route(DeleteContactPath::PATH, post(customer_detail::delete_contact))
        .route(CreateAddressPath::PATH, post(customer_detail::create_address))
        .route(DeleteAddressPath::PATH, post(customer_detail::delete_address))
        .route(CustomerTransactionsPath::PATH, get(customer_detail::get_customer_transactions))
        .route(DeleteCustomerPath::PATH, post(customer_list::delete_customer))
}
