use axum::routing::get;
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::state::AppState;

// ── TypedPath definitions ──
// 注：TypedPath struct 不 derive Serialize（会阻止 PATH 常量生成，见 abt-web/CLAUDE.md）

// 科目表（Chart of Accounts）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/accounts")]
pub struct GlAccountListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/accounts/create")]
pub struct GlAccountCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/accounts/{id}/toggle")]
pub struct GlAccountTogglePath {
    pub id: i64,
}

// 凭证（Journal Entries）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/entries")]
pub struct GlEntryListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/entries/{id}")]
pub struct GlEntryDetailPath {
    pub id: i64,
}

// 销售发票（Sales Invoices）
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/sales-invoices")]
pub struct SalesInvoiceListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/sales-invoices/create")]
pub struct SalesInvoiceCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/sales-invoices/{id}")]
pub struct SalesInvoiceDetailPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/sales-invoices/{id}/post")]
pub struct SalesInvoicePostPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/sales-invoices/{id}/cancel")]
pub struct SalesInvoiceCancelPath {
    pub id: i64,
}

// 试算 / 期间
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/trial-balance")]
pub struct GlTrialBalancePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/periods")]
pub struct GlPeriodListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/gl/periods/{id}/close")]
pub struct GlPeriodClosePath {
    pub id: i64,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        // 科目表
        .route(
            GlAccountListPath::PATH,
            get(crate::pages::gl_account_list::get_list),
        )
        .route(
            GlAccountCreatePath::PATH,
            get(crate::pages::gl_account_create::get_create)
                .post(crate::pages::gl_account_create::create),
        )
        .route(
            GlAccountTogglePath::PATH,
            axum::routing::post(crate::pages::gl_account_list::toggle_disabled),
        )
        // 凭证（Journal Entries）
        .route(
            GlEntryListPath::PATH,
            get(crate::pages::gl_entry_list::get_list),
        )
        .route(
            GlEntryDetailPath::PATH,
            get(crate::pages::gl_entry_detail::get_detail),
        )
        // 销售发票
        .route(
            SalesInvoiceListPath::PATH,
            get(crate::pages::sales_invoice_list::get_list),
        )
        .route(
            SalesInvoiceCreatePath::PATH,
            get(crate::pages::sales_invoice_create::get_create)
                .post(crate::pages::sales_invoice_create::create),
        )
        .route(
            SalesInvoiceDetailPath::PATH,
            get(crate::pages::sales_invoice_detail::get_detail),
        )
        .route(
            SalesInvoicePostPath::PATH,
            axum::routing::post(crate::pages::sales_invoice_detail::post),
        )
        .route(
            SalesInvoiceCancelPath::PATH,
            axum::routing::post(crate::pages::sales_invoice_detail::cancel),
        )
        // 试算/期间 等路由在后续 task（D4/D5）补
}
