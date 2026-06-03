use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::quotation_create;
use crate::pages::quotation_detail;
use crate::pages::quotation_edit;
use crate::pages::quotation_list;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations")]
pub struct QuotationListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/table")]
pub struct QuotationTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/new")]
pub struct QuotationCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}")]
pub struct QuotationDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}/edit-form")]
pub struct EditQuotationFormPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}/update")]
pub struct UpdateQuotationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}/delete")]
pub struct DeleteQuotationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}/submit")]
pub struct SubmitQuotationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}/accept")]
pub struct AcceptQuotationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/{id}/reject")]
pub struct RejectQuotationPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/products")]
pub struct QuotationProductsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/item-row")]
pub struct QuotationItemRowPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/quotations/customer-contacts")]
pub struct QuotationCustomerContactsPath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(QuotationListPath::PATH, get(quotation_list::get_quotation_list))
        .route(QuotationTablePath::PATH, get(quotation_list::get_quotation_table))
        .route(QuotationCreatePath::PATH, get(quotation_create::get_quotation_create).post(quotation_create::create_quotation))
        .route(QuotationDetailPath::PATH, get(quotation_detail::get_quotation_detail))
        .route(EditQuotationFormPath::PATH, get(quotation_edit::get_quotation_edit))
        .route(UpdateQuotationPath::PATH, post(quotation_edit::update_quotation))
        .route(DeleteQuotationPath::PATH, post(quotation_list::delete_quotation))
        .route(SubmitQuotationPath::PATH, post(quotation_detail::submit_quotation))
        .route(AcceptQuotationPath::PATH, post(quotation_detail::accept_quotation))
        .route(RejectQuotationPath::PATH, post(quotation_detail::reject_quotation))
        .route(QuotationProductsPath::PATH, get(quotation_create::get_products))
        .route(QuotationItemRowPath::PATH, get(quotation_create::get_quotation_item_row))
        .route(QuotationCustomerContactsPath::PATH, get(quotation_create::get_customer_contacts))
}
