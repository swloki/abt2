use axum::extract::Query;
use axum::Form;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::*;
use abt_core::sales::quotation::{QuotationService, model::{QuotationQuery, QuotationStatus}};
use abt_core::sales::sales_order::{SalesOrderService, model::{SalesOrderQuery, SalesOrderStatus}};
use abt_core::wms::outbound::{ShippingRequestService, model::{ShippingQuery, ShippingStatus}};
use abt_core::sales::sales_return::{SalesReturnService, model::{ReturnQuery, ReturnStatus}};
use abt_core::shared::types::{PageParams, PgExecutor};

use crate::components::icon;
use crate::components::pagination::htmx_pagination;
use crate::layout::page::admin_page;
use crate::routes::customer::{
 CreateAddressPath, CreateContactPath, CustomerDetailPath, CustomerListPath, CustomerTransactionsPath,
 DeleteAddressPath, DeleteContactPath,
};
use crate::state::AppState;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Transaction Record (unified view across sales sub-modules) ──

const TXN_PAGE_SIZE: u32 = 10;

enum TxType { Quotation, Order, Shipping, Return }

struct TransactionRecord {
 doc_number: String,
 tx_type: TxType,
 status_label: &'static str,
 status_class: &'static str,
 amount: Option<rust_decimal::Decimal>,
 date: chrono::NaiveDate,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TransactionQueryParams {
 page: Option<u32>,
}

// ── Transaction Data Fetching ──

async fn fetch_transactions(
 state: &AppState,
 service_ctx: &abt_core::shared::types::ServiceContext,
 db: PgExecutor<'_>,
 customer_id: i64,
) -> Vec<TransactionRecord> {
 let mut txns: Vec<TransactionRecord> = Vec::new();

 // Quotations
 let q_svc = state.quotation_service();
 if let Ok(page) = q_svc.list(service_ctx, db, QuotationQuery { customer_id: Some(customer_id), ..Default::default() }, PageParams::new(1, 200)).await {
 for q in &page.items {
 txns.push(TransactionRecord {
 doc_number: q.doc_number.clone(),
 tx_type: TxType::Quotation,
 status_label: match q.status { QuotationStatus::Draft => "草稿", QuotationStatus::Sent => "已发送", QuotationStatus::Accepted => "已接受", QuotationStatus::Rejected => "已拒绝", QuotationStatus::Expired => "已过期" },
 status_class: match q.status { QuotationStatus::Draft => "status-draft", QuotationStatus::Sent => "status-sent", QuotationStatus::Accepted => "status-accepted", QuotationStatus::Rejected => "status-rejected", QuotationStatus::Expired => "status-expired" },
 amount: Some(q.total_amount),
 date: q.created_at.date_naive(),
 });
 }
 }

 // Sales Orders
 let o_svc = state.sales_order_service();
 if let Ok(page) = o_svc.list(service_ctx, db, SalesOrderQuery { customer_id: Some(customer_id), ..Default::default() }, PageParams::new(1, 200)).await {
 for o in &page.items {
 txns.push(TransactionRecord {
 doc_number: o.doc_number.clone(),
 tx_type: TxType::Order,
 status_label: match o.status { SalesOrderStatus::Draft => "草稿", SalesOrderStatus::Confirmed => "已确认", SalesOrderStatus::ReadyToShip => "待发货", SalesOrderStatus::PartiallyShipped => "部分发货", SalesOrderStatus::Shipped => "已发货", SalesOrderStatus::Completed => "已完成", SalesOrderStatus::Cancelled => "已取消" },
 status_class: match o.status { SalesOrderStatus::Draft => "status-draft", SalesOrderStatus::Confirmed => "status-confirmed", SalesOrderStatus::ReadyToShip => "status-ready", SalesOrderStatus::PartiallyShipped => "status-partial", SalesOrderStatus::Shipped => "status-shipped", SalesOrderStatus::Completed => "status-completed", SalesOrderStatus::Cancelled => "status-cancelled" },
 amount: Some(o.total_amount),
 date: o.created_at.date_naive(),
 });
 }
 }

 // Shipping Requests
 let s_svc = state.shipping_service();
 if let Ok(page) = s_svc.list(service_ctx, db, ShippingQuery { customer_id: Some(customer_id), ..Default::default() }, PageParams::new(1, 200)).await {
 for s in &page.items {
 txns.push(TransactionRecord {
 doc_number: s.doc_number.clone(),
 tx_type: TxType::Shipping,
 status_label: match s.status { ShippingStatus::Draft => "草稿", ShippingStatus::Confirmed => "已确认", ShippingStatus::Picking => "拣货中", ShippingStatus::Shipped => "已发出", ShippingStatus::Cancelled => "已取消" },
 status_class: match s.status { ShippingStatus::Draft => "status-draft", ShippingStatus::Confirmed => "status-confirmed", ShippingStatus::Picking => "status-picking", ShippingStatus::Shipped => "status-shipped", ShippingStatus::Cancelled => "status-cancelled" },
 amount: None,
 date: s.created_at.date_naive(),
 });
 }
 }

 // Returns
 let r_svc = state.sales_return_service();
 if let Ok(page) = r_svc.list(service_ctx, db, ReturnQuery { customer_id: Some(customer_id), ..Default::default() }, PageParams::new(1, 200)).await {
 for r in &page.items {
 txns.push(TransactionRecord {
 doc_number: r.doc_number.clone(),
 tx_type: TxType::Return,
 status_label: match r.status { ReturnStatus::Draft => "草稿", ReturnStatus::Confirmed => "已确认", ReturnStatus::Received => "已收货", ReturnStatus::Inspecting => "质检中", ReturnStatus::Completed => "已完成", ReturnStatus::Cancelled => "已取消", ReturnStatus::Rejected => "已驳回" },
 status_class: match r.status { ReturnStatus::Draft => "status-draft", ReturnStatus::Confirmed => "status-confirmed", ReturnStatus::Received => "status-received", ReturnStatus::Inspecting => "status-inspecting", ReturnStatus::Completed => "status-completed", ReturnStatus::Cancelled => "status-cancelled", ReturnStatus::Rejected => "status-rejected" },
 amount: Some(r.total_amount),
 date: r.created_at.date_naive(),
 });
 }
 }

 // Sort by date descending
 txns.sort_by(|a, b| b.date.cmp(&a.date));
 txns
}

// ── Transaction Table Fragment (reused by detail page & HTMX endpoint) ──

fn transaction_table_fragment(
 all_txns: &[TransactionRecord],
 page: u32,
 page_size: u32,
 customer_id: i64,
) -> Markup {
 let total = all_txns.len() as u64;
 let total_pages = ((total as f64) / page_size as f64).ceil() as u32;
 let total_pages = total_pages.max(1);
 let page = page.min(total_pages);

 let offset = ((page - 1) * page_size) as usize;
 let page_txns: Vec<&TransactionRecord> = all_txns.iter().skip(offset).take(page_size as usize).collect();

 let txn_path = CustomerTransactionsPath { id: customer_id };

 html! {
    div class="bg-white border border-border-soft rounded p-5 mt-5 transaction-panel" {
        div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
        {
            span { "交易记录" }
        }
        @if page_txns.is_empty() {
            div class="text-center p-6 text-muted text-sm" { "暂无交易记录" }
        } @else {
            table class="data-table" {
                thead {
                    tr {
                        th { "单据编号" }
                        th { "类型" }
                        th { "状态" }
                        th class="text-right text-[13px]" { "金额" }
                        th { "日期" }
                    }
                }
                tbody {
                    @for tx in &page_txns {
                        tr {
                            td.mono { (tx.doc_number) }
                            td {
                                ({
                                    match tx.tx_type {
                                        TxType::Quotation => "报价单",
                                        TxType::Order => "销售订单",
                                        TxType::Shipping => "发货申请",
                                        TxType::Return => "退货单",
                                    }
                                })
                            }
                            td {
                                span
                                    class=({
                                        format!(
                                            "status-pill {}",
                                            crate::utils::status_color(tx.status_class),
                                        )
                                    })
                                { (tx.status_label) }
                            }
                            td.mono.num-right {
                                @if let Some(amt) = tx.amount { (crate::utils::fmt_amount(amt)) } @else {
                                    "—"
                                }
                            }
                            td { (tx.date) }
                        }
                    }
                }
            }
            ({
                htmx_pagination(
                    txn_path.to_string().as_str(),
                    "",
                    total,
                    page,
                    total_pages,
                    "closest .transaction-panel",
                    "outerHTML",
                )
            })
        }
    }
}
}

// ── Handlers ──

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_detail(
 path: CustomerDetailPath,
 ctx: RequestContext,

) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let can_delete = ctx.has_permission("CUSTOMER", "delete").await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.customer_service();
 let cid = path.id;

 let customer = svc.get(&service_ctx, &mut conn, cid).await?;
 let contacts = svc.list_contacts(&service_ctx, &mut conn, cid).await?;
 let addresses = svc.list_addresses(&service_ctx, &mut conn, cid).await?;

 // ── Fetch transaction history (paginated across all sales sub-modules) ──
 let txns = fetch_transactions(&state, &service_ctx, &mut conn, cid).await;
 let txn_html = transaction_table_fragment(&txns, 1, TXN_PAGE_SIZE, cid);

 let content = customer_detail_page(&customer, &contacts, &addresses, txn_html, can_delete);
 let detail_path_str = CustomerDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 客户详情", customer.name),
 &claims,
 "sales",
 &detail_path_str,
 "销售管理",
 Some(&customer.name),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_transactions(
 path: CustomerTransactionsPath,
 ctx: RequestContext,
 Query(params): Query<TransactionQueryParams>,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let cid = path.id;
 let page = params.page.unwrap_or(1);

 let txns = fetch_transactions(&state, &service_ctx, &mut conn, cid).await;
 let html = transaction_table_fragment(&txns, page, TXN_PAGE_SIZE, cid);

 Ok(Html(html.into_string()))
}

#[require_permission("CUSTOMER", "create")]
pub async fn create_contact(
 path: CreateContactPath,
 ctx: RequestContext,
 Form(form): Form<ContactForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.customer_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let req = CreateContactReq {
 contact_name: form.contact_name,
 phone: form.phone,
 email: form.email,
 position: form.position,
 fax: None,
 fixed_phone: None,
 is_primary: form.is_primary.unwrap_or(false),
 };

 svc.add_contact(&service_ctx, &mut tx, path.id, req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = format!("/admin/customers/{}", path.id);
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("CUSTOMER", "delete")]
pub async fn delete_contact(
 path: DeleteContactPath,
 ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.customer_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 svc.delete_contact(&service_ctx, &mut tx, path.cid, path.contact_id)
 .await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let detail = CustomerDetailPath { id: path.cid };
 Ok(([("HX-Redirect", detail.to_string())], Html(String::new())))
}

#[require_permission("CUSTOMER", "create")]
pub async fn create_address(
 path: CreateAddressPath,
 ctx: RequestContext,
 Form(form): Form<AddressForm>,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.customer_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let req = CreateAddressReq {
 address_type: form.address_type,
 province: form.province,
 city: form.city,
 district: form.district,
 detail: form.detail,
 contact_name: form.contact_name,
 contact_phone: form.contact_phone,
 is_default: form.is_default.unwrap_or(false),
 };

 svc.add_address(&service_ctx, &mut tx, path.id, req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = format!("/admin/customers/{}", path.id);
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("CUSTOMER", "delete")]
pub async fn delete_address(
 path: DeleteAddressPath,
 ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let svc = state.customer_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 svc.delete_address(&service_ctx, &mut tx, path.cid, path.address_id)
 .await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let detail = CustomerDetailPath { id: path.cid };
 Ok(([("HX-Redirect", detail.to_string())], Html(String::new())))
}

// ── Helpers ──

fn avatar_chars(name: &str) -> &str {
 let n = name.trim();
 if n.is_empty() {
 return "??";
 }
 let end = n.char_indices().nth(2).map_or(n.len(), |(i, _)| i);
 &n[..end]
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub(crate) struct ContactForm {
 contact_name: String,
 phone: Option<String>,
 email: Option<String>,
 position: Option<String>,
 is_primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddressForm {
 address_type: String,
 province: String,
 city: String,
 district: Option<String>,
 detail: String,
 contact_name: Option<String>,
 contact_phone: Option<String>,
 is_default: Option<bool>,
}

// ── Components ──

fn customer_detail_page(
 customer: &Customer,
 contacts: &[CustomerContact],
 addresses: &[CustomerAddress],
 txn_html: Markup,
 can_delete: bool,
) -> Markup {
 let detail_path = CustomerDetailPath { id: customer.id };
 let list_path = CustomerListPath;
 let contact_create_path = CreateContactPath { id: customer.id };
 let address_create_path = CreateAddressPath { id: customer.id };

 let category_label = match customer.category {
 CustomerCategory::Distributor => "经销商",
 CustomerCategory::DirectCustomer => "直客",
 CustomerCategory::OEM => "OEM",
 CustomerCategory::Retailer => "零售商",
 };
 let (status_label, status_class) = match customer.status {
 CustomerStatus::Prospective => ("潜在客户", "status-draft"),
 CustomerStatus::Active => ("活跃", "status-accepted"),
 CustomerStatus::Inactive => ("已停用", "status-rejected"),
 CustomerStatus::Blacklisted => ("黑名单", "status-rejected"),
 };

 html! {
    div {
        // ── Back Link ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{list_path}?restore=true"))
        { (icon::arrow_left_icon("w-4 h-4")) "返回客户列表" }
        // ── Detail Top ──
        div class="flex justify-between items-start" {
            div class="flex items-center gap-5" {
                div class="w-10 h-10 grid place-items-center rounded-full bg-accent text-white font-semibold shrink-0 select-none"
                { (avatar_chars(&customer.name)) }
                div {
                    h1 class="text-xl font-bold" {
                        (customer.name)
                        " "
                        span class="bg-accent-bg text-accent rounded-full text-[11px] font-medium" {
                            (category_label)
                        }
                    }
                    div class="flex gap-4 text-muted text-xs" {
                        span { (customer.code) }
                        span { (customer.created_at.format("%Y-%m-%d")) }
                    }
                }
            }
            div class="flex gap-3" {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    href=(format!("/admin/quotations/new"))
                { "新建报价单" }
            }
        }
        // ── 3-Column Detail Grid ──
        div class="grid gap-5" {
            // ── Left: Basic Info ──
            div class="bg-white border border-border-soft rounded p-5" {
                div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
                { "基本信息" }
                div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-x-6" {
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "客户全称" }
                    span class="detail-value" { (customer.name) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "客户简称" }
                    span class="detail-value" { (customer.short_name.as_deref().unwrap_or("—")) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "客户编码" }
                    span class="detail-value font-mono tabular-nums" { (customer.code) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "客户分类" }
                    span class="detail-value" { (category_label) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "状态" }
                    span class="detail-value" {
                        span
                            class=({
                                format!(
                                    "status-pill {}",
                                    crate::utils::status_color(status_class),
                                )
                            })
                        { (status_label) }
                    }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "付款条款" }
                    span class="detail-value" { (customer.payment_terms.as_deref().unwrap_or("—")) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "发票抬头" }
                    span class="detail-value" { (customer.invoice_title.as_deref().unwrap_or("—")) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "创建时间" }
                    span class="detail-value" { (customer.created_at.format("%Y-%m-%d")) }
                }
                div class="flex py-2 text-sm" {
                    span class="w-[90px] shrink-0 text-muted" { "备注" }
                    span class="detail-value" {
                        @if customer.remark.is_empty() { "—" } @else { (&customer.remark) }
                    }
                }
                }
            }
            // ── Center: Contacts ──
            div class="bg-white border border-border-soft rounded p-5" {
                div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
                {
                    span { "联系人" }
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                        _="on click add .is-open to #contact-create-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加" }
                }
                @if contacts.is_empty() {
                    div class="text-center p-6 text-muted text-sm" { "暂无联系人" }
                } @else {
                    @for c in contacts { (contact_card(c, &detail_path, can_delete)) }
                }
            }
            // ── Right: Credit & Financial ──
            div class="bg-white border border-border-soft rounded p-5" {
                div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
                { "信用额度" }
                (credit_display(customer.credit_limit))
                div class="border-t border-border-soft pt-4" {
                    div class="flex py-2 text-sm" {
                        span class="w-[90px] shrink-0 text-muted" { "付款条款" }
                        span class="detail-value" {
                            (customer.payment_terms.as_deref().unwrap_or("—"))
                        }
                    }
                    div class="flex py-2 text-sm" {
                        span class="w-[90px] shrink-0 text-muted" { "税号" }
                        span class="detail-value font-mono tabular-nums text-xs" {
                            (customer.tax_number.as_deref().unwrap_or("—"))
                        }
                    }
                }
            }
        }
        // ── Addresses Section (full width) ──
        div class="bg-white border border-border-soft rounded p-5 mt-5" {
            div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
            {
                span { "地址信息" }
                button
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                    _="on click add .is-open to #address-create-modal"
                { (icon::plus_icon("w-3.5 h-3.5")) "添加" }
            }
            @if addresses.is_empty() {
                div class="text-center p-6 text-muted text-sm" { "暂无地址" }
            } @else {
                div class="grid gap-3" {
                    @for a in addresses { (address_card(a, &detail_path, can_delete)) }
                }
            }
        }
        // ── Transaction History ──
        (txn_html)
        // ── Modals ──
        ({
            crate::components::modal::modal(
                "contact-create-modal",
                "添加联系人",
                "保存",
                "create-contact-form",
                &contact_create_path.to_string(),
                html! {
                    div class = "grid grid-cols-2 gap-4 gap-x-6 mb-6" { div class =
                    "form-field" { label { "姓名 *" } input type = "text" name =
                    "contact_name" required placeholder = "请输入联系人姓名"; }
                    div class = "form-field" { label { "职位" } input type = "text"
                    name = "position" placeholder = "请输入职位"; } div class =
                    "form-field" { label { "电话" } input type = "text" name = "phone"
                    placeholder = "请输入电话"; } div class = "form-field" { label {
                    "邮箱" } input type = "email" name = "email" placeholder =
                    "请输入邮箱"; } div class = "form-field" { label class =
                    "checkbox-label" { input type = "checkbox" name = "is_primary" value
                    = "true"; "主要联系人" } } }
                },
            )
        })

        ({
            crate::components::modal::modal(
                "address-create-modal",
                "添加地址",
                "保存",
                "create-address-form",
                &address_create_path.to_string(),
                html! {
                    div class = "grid grid-cols-2 gap-4 gap-x-6 mb-6" { div class =
                    "form-field" { label { "地址类型 *" } select name =
                    "address_type" { option value = "shipping" { "收货地址" } option
                    value = "billing" { "开票地址" } option value = "other" {
                    "其他" } } } div class = "form-field" { label { "省份 *" } input
                    type = "text" name = "province" required placeholder =
                    "请输入省份"; } div class = "form-field" { label { "城市 *" }
                    input type = "text" name = "city" required placeholder =
                    "请输入城市"; } div class = "form-field" { label { "区县" }
                    input type = "text" name = "district" placeholder =
                    "请输入区县"; } div class = "form-field field-full" { label {
                    "详细地址 *" } input type = "text" name = "detail" required
                    placeholder = "请输入详细地址"; } div class = "form-field" {
                    label { "收件人" } input type = "text" name = "contact_name"
                    placeholder = "请输入收件人"; } div class = "form-field" {
                    label { "联系电话" } input type = "text" name = "contact_phone"
                    placeholder = "请输入联系电话"; } div class = "form-field" {
                    label class = "checkbox-label" { input type = "checkbox" name =
                    "is_default" value = "true"; "默认地址" } } }
                },
            )
        })
    }
}
}

fn credit_display(credit_limit: Option<rust_decimal::Decimal>) -> Markup {
 html! {
    div class="text-center p-5" {
        @if let Some(limit) = credit_limit {
            // 仅有总额度，无已用数据 → 空环 + "未设置"提示
            div class="w-[120px] h-[120px] relative mx-auto" {
                svg viewBox="0 0 120 120" class="w-[120px] h-[120px] -rotate-90" {
                    circle
                        cx="60"
                        cy="60"
                        r="50"
                        fill="none"
                        stroke="var(--border-soft)"
                        stroke-width="10" {}
                    circle
                        cx="60"
                        cy="60"
                        r="50"
                        fill="none"
                        stroke="var(--accent)"
                        stroke-width="10"
                        stroke-dasharray="314.16"
                        stroke-dashoffset="314.16"
                        stroke-linecap="round" {}
                }
                div class="absolute inset-0 grid place-items-center" {
                    div class="flex flex-col items-center" {
                        div class="text-muted text-sm" { "—" }
                        div class="text-xs text-muted" { "未设置" }
                    }
                }
            }
            div class="text-xs text-muted mt-2" { "总额度" }
            div class="text-lg font-bold" { (crate::utils::fmt_amount(limit)) }
        } @else {
            div class="w-[120px] h-[120px] relative mx-auto" {
                svg viewBox="0 0 120 120" class="w-[120px] h-[120px] -rotate-90" {
                    circle
                        cx="60"
                        cy="60"
                        r="50"
                        fill="none"
                        stroke="var(--border-soft)"
                        stroke-width="10" {}
                }
                div class="absolute inset-0 grid place-items-center" {
                    div class="flex flex-col items-center" {
                        div class="text-muted text-sm" { "—" }
                        div class="text-xs text-muted" { "未设置" }
                    }
                }
            }
            div class="text-xs text-muted mt-2" { "未设置信用额度" }
        }
    }
}
}

fn contact_card(contact: &CustomerContact, detail_path: &CustomerDetailPath, can_delete: bool) -> Markup {
 let delete_path = DeleteContactPath {
 cid: detail_path.id,
 contact_id: contact.id,
 };

 html! {
    div class="p-3 border border-border-soft rounded-sm" {
        div class="flex items-center gap-2 p-3 border border-border-soft rounded-sm" {
            strong { (contact.name) }
            @if contact.is_primary {
                span
                    class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-accent-bg text-accent"
                { "主要" }
            }
            @if let Some(ref pos) = contact.position {
                span
                    class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-surface text-slate-500"
                { (pos) }
            }
        }
        div class="p-3" {
            @if let Some(ref phone) = contact.phone {
                div class="flex items-center gap-2 text-xs text-fg-2" {
                    (icon::phone_icon("w-3.5 h-3.5"))
                    span { (phone) }
                }
            }
            @if let Some(ref email) = contact.email {
                div class="flex items-center gap-2 text-xs text-fg-2" {
                    (icon::mail_icon("w-3.5 h-3.5"))
                    span { (email) }
                }
            }
        }
        div class="p-3 flex justify-end" {
            @if can_delete {
                button
                    type="button"
                    class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger"
                    title="删除"
                    hx-post=(delete_path)
                    hx-confirm=({
                        format!(
                            "删除后无法恢复，确定要删除联系人 <strong>{}</strong> 吗？",
                            contact.name,
                        )
                    })
                    hx-swap="none"
                { (icon::trash_icon("w-4 h-4")) }
            }
        }
    }
}
}

fn address_card(addr: &CustomerAddress, detail_path: &CustomerDetailPath, can_delete: bool) -> Markup {
 let delete_path = DeleteAddressPath {
 cid: detail_path.id,
 address_id: addr.id,
 };
 let type_label = match addr.address_type.as_str() {
 "shipping" => "收货",
 "billing" => "开票",
 _ => "其他",
 };
 let full_addr = format!(
 "{}{}{}{}",
 addr.province,
 addr.city,
 addr.district.as_deref().unwrap_or(""),
 addr.detail,
 );

 html! {
    div class="p-3 border border-border-soft rounded-sm" {
        div class="flex items-center gap-2 p-3 border border-border-soft rounded-sm" {
            span
                class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-surface text-slate-500"
            { (type_label) }
            @if addr.is_default {
                span
                    class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-accent-bg text-accent"
                { "默认" }
            }
        }
        div class="p-3" {
            p { (full_addr) }
            @if let Some(ref name) = addr.contact_name {
                p class="flex items-center gap-2 text-xs text-muted" {
                    (icon::user_icon("w-3.5 h-3.5"))
                    span { (name) }
                    @if let Some(ref phone) = addr.contact_phone {
                        span { " " (phone) }
                    }
                }
            }
        }
        div class="p-3 flex justify-end" {
            @if can_delete {
                button
                    type="button"
                    class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger"
                    title="删除"
                    hx-post=(delete_path)
                    hx-confirm="删除后无法恢复，确定要删除该地址吗？"
                    hx-swap="none"
                { (icon::trash_icon("w-4 h-4")) }
            }
        }
    }
}
}
