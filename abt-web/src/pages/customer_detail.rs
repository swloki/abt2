use axum::Form;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::*;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::customer::{
    CreateAddressPath, CreateContactPath, CustomerDetailPath, CustomerListPath, DeleteAddressPath,
    DeleteContactPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_detail(
    path: CustomerDetailPath,
    ctx: RequestContext,

) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.customer_service();

    let customer = svc.get(&service_ctx, &mut conn, path.id).await?;

    let contacts = svc.list_contacts(&service_ctx, &mut conn, path.id).await?;

    let addresses = svc.list_addresses(&service_ctx, &mut conn, path.id).await?;

    let content = customer_detail_page(&customer, &contacts, &addresses);
    let detail_path_str = CustomerDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 客户详情", customer.name),
        &claims,
        "sales",
        &detail_path_str,
        "销售管理",
        Some(&customer.name),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("CUSTOMER", "create")]
pub async fn create_contact(
    path: CreateContactPath,
    ctx: RequestContext,
    Form(form): Form<ContactForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    let req = CreateContactReq {
        contact_name: form.contact_name,
        phone: form.phone,
        email: form.email,
        position: form.position,
        is_primary: form.is_primary.unwrap_or(false),
    };

    svc.add_contact(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = format!("/admin/customers/{}", path.id);
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("CUSTOMER", "delete")]
pub async fn delete_contact(
    path: DeleteContactPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    svc.delete_contact(&service_ctx, &mut conn, path.cid, path.contact_id)
        .await?;

    let detail = CustomerDetailPath { id: path.cid };
    Ok(([("HX-Redirect", detail.to_string())], Html(String::new())))
}

#[require_permission("CUSTOMER", "create")]
pub async fn create_address(
    path: CreateAddressPath,
    ctx: RequestContext,
    Form(form): Form<AddressForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

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

    svc.add_address(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = format!("/admin/customers/{}", path.id);
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("CUSTOMER", "delete")]
pub async fn delete_address(
    path: DeleteAddressPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    svc.delete_address(&service_ctx, &mut conn, path.cid, path.address_id)
        .await?;

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
        // ── Detail Top ──
        div class="detail-top" {
            div class="customer-identity" {
                div class="customer-avatar" { (avatar_chars(&customer.name)) }
                div {
                    h1 class="customer-name" {
                        (customer.name)
                        " "
                        span class="tag-key" { (category_label) }
                    }
                    div class="customer-meta" {
                        span { (customer.code) }
                        span { (customer.created_at.format("%Y-%m-%d")) }
                    }
                }
            }
            div class="page-actions" {
                a class="btn btn-default" href=(list_path) { "返回列表" }
                a class="btn btn-primary" href="#" { "新建报价单" }
            }
        }

        // ── 3-Column Detail Grid ──
        div class="detail-grid" {
            // ── Left: Basic Info ──
            div class="detail-card" {
                div class="detail-card-title" { "基本信息" }
                div class="detail-row" {
                    span class="detail-label" { "客户全称" }
                    span class="detail-value" { (customer.name) }
                }
                div class="detail-row" {
                    span class="detail-label" { "客户简称" }
                    span class="detail-value" { (customer.short_name.as_deref().unwrap_or("—")) }
                }
                div class="detail-row" {
                    span class="detail-label" { "客户编码" }
                    span class="detail-value mono" { (customer.code) }
                }
                div class="detail-row" {
                    span class="detail-label" { "客户分类" }
                    span class="detail-value" { (category_label) }
                }
                div class="detail-row" {
                    span class="detail-label" { "状态" }
                    span class="detail-value" {
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="detail-row" {
                    span class="detail-label" { "付款条款" }
                    span class="detail-value" { (customer.payment_terms.as_deref().unwrap_or("—")) }
                }
                div class="detail-row" {
                    span class="detail-label" { "发票抬头" }
                    span class="detail-value" { (customer.invoice_title.as_deref().unwrap_or("—")) }
                }
                div class="detail-row" {
                    span class="detail-label" { "创建时间" }
                    span class="detail-value" { (customer.created_at.format("%Y-%m-%d")) }
                }
                div class="detail-row" {
                    span class="detail-label" { "备注" }
                    span class="detail-value" {
                        @if customer.remark.is_empty() { "—" } @else { (&customer.remark) }
                    }
                }
            }

            // ── Center: Contacts ──
            div class="detail-card" {
                div class="detail-card-title" {
                    span { "联系人" }
                    button class="btn btn-sm btn-primary"
                        onclick="hsAdd(null,'#contact-create-modal','is-open')" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加"
                    }
                }
                @if contacts.is_empty() {
                    div class="empty-state" { "暂无联系人" }
                } @else {
                    @for c in contacts {
                        (contact_card(c, &detail_path))
                    }
                }
            }

            // ── Right: Credit & Financial ──
            div class="detail-card" {
                div class="detail-card-title" { "信用额度" }
                (credit_display(customer.credit_limit))
                div style="border-top:1px solid var(--border-soft);padding-top:var(--space-4)" {
                    div class="detail-row" {
                        span class="detail-label" { "付款条款" }
                        span class="detail-value" { (customer.payment_terms.as_deref().unwrap_or("—")) }
                    }
                    div class="detail-row" {
                        span class="detail-label" { "税号" }
                        span class="detail-value mono" style="font-size:12px" {
                            (customer.tax_number.as_deref().unwrap_or("—"))
                        }
                    }
                }
            }
        }

        // ── Addresses Section (full width) ──
        div class="detail-card" style="margin-top:var(--space-5)" {
            div class="detail-card-title" {
                span { "地址信息" }
                button class="btn btn-sm btn-primary"
                    onclick="hsAdd(null,'#address-create-modal','is-open')" {
                    (icon::plus_icon("w-3.5 h-3.5"))
                    "添加"
                }
            }
            @if addresses.is_empty() {
                div class="empty-state" { "暂无地址" }
            } @else {
                div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-3)" {
                    @for a in addresses {
                        (address_card(a, &detail_path))
                    }
                }
            }
        }

        // ── Modals ──
            (crate::components::modal::modal(
                "contact-create-modal",
                "添加联系人",
                "保存",
                "create-contact-form",
                &contact_create_path.to_string(),
                html! {
                    div class="form-grid" {
                        div class="form-field" {
                            label { "姓名 *" }
                            input type="text" name="contact_name" required placeholder="请输入联系人姓名";
                        }
                        div class="form-field" {
                            label { "职位" }
                            input type="text" name="position" placeholder="请输入职位";
                        }
                        div class="form-field" {
                            label { "电话" }
                            input type="text" name="phone" placeholder="请输入电话";
                        }
                        div class="form-field" {
                            label { "邮箱" }
                            input type="email" name="email" placeholder="请输入邮箱";
                        }
                        div class="form-field" {
                            label class="checkbox-label" {
                                input type="checkbox" name="is_primary" value="true";
                                "主要联系人"
                            }
                        }
                    }
                },
            ))

            (crate::components::modal::modal(
                "address-create-modal",
                "添加地址",
                "保存",
                "create-address-form",
                &address_create_path.to_string(),
                html! {
                    div class="form-grid" {
                        div class="form-field" {
                            label { "地址类型 *" }
                            select name="address_type" {
                                option value="shipping" { "收货地址" }
                                option value="billing" { "开票地址" }
                                option value="other" { "其他" }
                            }
                        }
                        div class="form-field" {
                            label { "省份 *" }
                            input type="text" name="province" required placeholder="请输入省份";
                        }
                        div class="form-field" {
                            label { "城市 *" }
                            input type="text" name="city" required placeholder="请输入城市";
                        }
                        div class="form-field" {
                            label { "区县" }
                            input type="text" name="district" placeholder="请输入区县";
                        }
                        div class="form-field field-full" {
                            label { "详细地址 *" }
                            input type="text" name="detail" required placeholder="请输入详细地址";
                        }
                        div class="form-field" {
                            label { "收件人" }
                            input type="text" name="contact_name" placeholder="请输入收件人";
                        }
                        div class="form-field" {
                            label { "联系电话" }
                            input type="text" name="contact_phone" placeholder="请输入联系电话";
                        }
                        div class="form-field" {
                            label class="checkbox-label" {
                                input type="checkbox" name="is_default" value="true";
                                "默认地址"
                            }
                        }
                    }
                },
            ))
        }
    }
}

fn credit_display(credit_limit: Option<rust_decimal::Decimal>) -> Markup {
    html! {
        div class="credit-display" {
            @if let Some(limit) = credit_limit {
                div class="credit-ring" {
                    svg viewBox="0 0 120 120" {
                        circle cx="60" cy="60" r="50" fill="none" stroke="var(--border-soft)" stroke-width="10" {}
                        circle cx="60" cy="60" r="50" fill="none" stroke="var(--accent)" stroke-width="10"
                            stroke-dasharray="314.16" stroke-dashoffset="314.16" stroke-linecap="round" {}
                    }
                    div class="credit-ring-text" {
                        div class="credit-ring-value" style="color:var(--muted)" { "—" }
                        div class="credit-ring-label" { "已用额度" }
                    }
                }
                div style="font-size:var(--text-xs);color:var(--muted);margin-bottom:var(--space-1)" { "总额度" }
                div style="font-size:var(--text-lg);font-weight:700" {
                    "¥ " (format!("{:.2}", limit))
                }
            } @else {
                div class="credit-ring" {
                    svg viewBox="0 0 120 120" {
                        circle cx="60" cy="60" r="50" fill="none" stroke="var(--border-soft)" stroke-width="10" {}
                    }
                    div class="credit-ring-text" {
                        div class="credit-ring-value" style="color:var(--muted)" { "—" }
                        div class="credit-ring-label" { "未设置" }
                    }
                }
                div style="font-size:var(--text-xs);color:var(--muted)" { "未设置信用额度" }
            }
        }
    }
}

fn contact_card(contact: &CustomerContact, detail_path: &CustomerDetailPath) -> Markup {
    let delete_path = DeleteContactPath {
        cid: detail_path.id,
        contact_id: contact.id,
    };

    html! {
        div class="contact-card" {
            div class="contact-card-head" {
                strong { (contact.name) }
                @if contact.is_primary {
                    span class="tag-chip tag-key" { "主要" }
                }
                @if let Some(ref pos) = contact.position {
                    span class="tag-chip tag-normal" { (pos) }
                }
            }
            div class="contact-card-body" {
                @if let Some(ref phone) = contact.phone {
                    div class="contact-info-row" {
                        (icon::phone_icon("w-3.5 h-3.5"))
                        span { (phone) }
                    }
                }
                @if let Some(ref email) = contact.email {
                    div class="contact-info-row" {
                        (icon::mail_icon("w-3.5 h-3.5"))
                        span { (email) }
                    }
                }
            }
            div class="contact-card-actions" {
                button type="button" class="row-action-btn text-danger" title="删除"
                    hx-post=(delete_path)
                    hx-confirm=(format!("删除后无法恢复，确定要删除联系人 <strong>{}</strong> 吗？", contact.name))
                    hx-swap="none" {
                    (icon::trash_icon("w-4 h-4"))
                }
            }
        }
    }
}

fn address_card(addr: &CustomerAddress, detail_path: &CustomerDetailPath) -> Markup {
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
        div class="address-card" {
            div class="address-card-head" {
                span class="tag-chip tag-normal" { (type_label) }
                @if addr.is_default {
                    span class="tag-chip tag-key" { "默认" }
                }
            }
            div class="address-card-body" {
                p { (full_addr) }
                @if let Some(ref name) = addr.contact_name {
                    p class="address-contact" {
                        (icon::user_icon("w-3.5 h-3.5"))
                        span { (name) }
                        @if let Some(ref phone) = addr.contact_phone {
                            span { " " (phone) }
                        }
                    }
                }
            }
            div class="address-card-actions" {
                button type="button" class="row-action-btn text-danger" title="删除"
                    hx-post=(delete_path)
                    hx-confirm="删除后无法恢复，确定要删除该地址吗？"
                    hx-swap="none" {
                    (icon::trash_icon("w-4 h-4"))
                }
            }
        }
    }
}
