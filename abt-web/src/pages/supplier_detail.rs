use axum::Form;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
 use serde::Deserialize;

use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::*;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::supplier::{
    SupplierContactPath, SupplierDeleteContactPath, SupplierDeletePath,
    SupplierDeleteBankAccountPath, SupplierDetailPath, SupplierListPath, SupplierBankAccountPath,
    SupplierEditPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("SUPPLIER", "read")]
pub async fn get_supplier_detail(
    path: SupplierDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_delete = ctx.has_permission("SUPPLIER", "delete").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.supplier_service();

    let supplier = svc.get(&service_ctx, &mut conn, path.id).await?;
    let contacts = svc.list_contacts(&service_ctx, &mut conn, path.id).await?;
    let bank_accounts = svc.list_bank_accounts(&service_ctx, &mut conn, path.id).await?;

    let content = supplier_detail_page(&supplier, &contacts, &bank_accounts, can_delete);
    let detail_path_str = SupplierDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 供应商详情", supplier.name),
        &claims,
        "purchase",
        &detail_path_str,
        "主数据管理",
        Some(&supplier.name),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SUPPLIER", "create")]
pub async fn create_supplier_contact(
    path: SupplierContactPath,
    ctx: RequestContext,
    Form(form): Form<ContactForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_service();

    let req = CreateContactReq {
        contact_name: form.contact_name,
        phone: form.phone,
        email: form.email,
        position: form.position,
        is_primary: form.is_primary.unwrap_or(false),
    };

    svc.add_contact(&service_ctx, &mut conn, path.id, req).await?;
    Ok((StatusCode::OK, [("HX-Trigger", "contactChanged")], Html(String::new())))
}

#[require_permission("SUPPLIER", "delete")]
pub async fn delete_supplier_contact(
    path: SupplierDeleteContactPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_service();

    svc.delete_contact(&service_ctx, &mut conn, path.sid, path.contact_id)
        .await?;
    Ok((StatusCode::OK, [("HX-Trigger", "contactChanged")], Html(String::new())))
}

#[require_permission("SUPPLIER", "create")]
pub async fn create_supplier_bank_account(
    path: SupplierBankAccountPath,
    ctx: RequestContext,
    Form(form): Form<BankAccountForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_service();

    let req = CreateBankAccountReq {
        bank_name: form.bank_name,
        account_name: form.account_name,
        account_number: form.account_number,
        is_default: form.is_default.unwrap_or(false),
    };

    svc.add_bank_account(&service_ctx, &mut conn, path.id, req).await?;
    Ok((StatusCode::OK, [("HX-Trigger", "bankAccountChanged")], Html(String::new())))
}

#[require_permission("SUPPLIER", "delete")]
pub async fn delete_supplier_bank_account(
    path: SupplierDeleteBankAccountPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_service();

    svc.delete_bank_account(&service_ctx, &mut conn, path.sid, path.account_id)
        .await?;
    Ok((StatusCode::OK, [("HX-Trigger", "bankAccountChanged")], Html(String::new())))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub(crate) struct ContactForm {
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BankAccountForm {
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    pub is_default: Option<bool>,
}

// ── Components ──

fn supplier_detail_page(
    supplier: &Supplier,
    contacts: &[SupplierContact],
    bank_accounts: &[SupplierBankAccount],
    can_delete: bool,
) -> Markup {
    let detail_path = SupplierDetailPath { id: supplier.id };
    let list_path = SupplierListPath;
    let contact_create_path = SupplierContactPath { id: supplier.id };
    let bank_account_create_path = SupplierBankAccountPath { id: supplier.id };
    let edit_path = SupplierEditPath { id: supplier.id };
    let _delete_path = SupplierDeletePath { id: supplier.id };

    let category_label = match supplier.category {
        SupplierCategory::RawMaterial => "原材料",
        SupplierCategory::Packaging => "包装材料",
        SupplierCategory::Outsourcing => "外协加工",
        SupplierCategory::Consumable => "辅料",
        SupplierCategory::Service => "服务",
    };
    let (status_label, status_class) = match supplier.status {
        SupplierStatus::Prospective => ("潜在", "status-draft"),
        SupplierStatus::Qualified => ("合格", "status-accepted"),
        SupplierStatus::Probation => ("试用期", "status-progress"),
        SupplierStatus::Disqualified => ("不合格", "status-rejected"),
        SupplierStatus::Blacklisted => ("黑名单", "status-rejected"),
    };

    html! {
        div _={"on contactChanged from the body remove .is-open from #contact-create-modal\non bankAccountChanged from the body remove .is-open from #bank-account-create-modal"} {
        // ── Detail Top ──
        div class="detail-top" {
            div class="customer-identity" {
                div class="customer-avatar" {
                    (icon::building_icon("w-5 h-5"))
                }
                div {
                    h1 class="customer-name" {
                        (supplier.name)
                        " "
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                    div class="customer-meta" {
                        span { (supplier.code) }
                        span { (category_label) }
                        span { (supplier.created_at.format("%Y-%m-%d")) }
                    }
                }
            }
            div class="page-actions" {
                a class="btn btn-default" href=(list_path) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    " 返回列表"
                }
                a class="btn btn-primary" href=(edit_path) {
                    (icon::edit_icon("w-4 h-4"))
                    " 编辑"
                }
                // TODO: status change and delete actions
            }
        }

        // ── Basic Info Card ──
        div class="detail-card" style="margin-bottom:var(--space-5)" {
            div class="detail-card-title" { "基本信息" }
            div class="detail-row" {
                span class="detail-label" { "供应商编码" }
                span class="detail-value mono" { (supplier.code) }
            }
            div class="detail-row" {
                span class="detail-label" { "供应商名称" }
                span class="detail-value" { (supplier.name) }
            }
            div class="detail-row" {
                span class="detail-label" { "简称" }
                span class="detail-value" { (supplier.short_name.as_deref().unwrap_or("—")) }
            }
            div class="detail-row" {
                span class="detail-label" { "类别" }
                span class="detail-value" { (category_label) }
            }
            div class="detail-row" {
                span class="detail-label" { "状态" }
                span class="detail-value" {
                    span class=(format!("status-pill {status_class}")) { (status_label) }
                }
            }
            div class="detail-row" {
                span class="detail-label" { "税号" }
                span class="detail-value mono" style="font-size:12px" {
                    (supplier.tax_number.as_deref().unwrap_or("—"))
                }
            }
            div class="detail-row" {
                span class="detail-label" { "交货天数" }
                span class="detail-value" {
                    @if supplier.lead_time_days > 0 {
                        (supplier.lead_time_days) " 天"
                    } @else {
                        "—"
                    }
                }
            }
            div class="detail-row" {
                span class="detail-label" { "付款条件" }
                span class="detail-value" { (supplier.payment_terms.as_deref().unwrap_or("—")) }
            }
            div class="detail-row" {
                span class="detail-label" { "结算货币" }
                span class="detail-value" { (&supplier.currency) }
            }
            div class="detail-row" {
                span class="detail-label" { "创建时间" }
                span class="detail-value" { (supplier.created_at.format("%Y-%m-%d")) }
            }
            div class="detail-row" {
                span class="detail-label" { "备注" }
                span class="detail-value" {
                    @if supplier.remark.is_empty() { "—" } @else { (&supplier.remark) }
                }
            }
        }

        // ── 2-Column Grid: Contacts + Bank Accounts ──
        div style="display:grid;grid-template-columns:1fr;gap:var(--space-5)" {
            // ── Contacts Card ──
            (contacts_card(contacts, &detail_path, can_delete))
            (bank_accounts_card(bank_accounts, &detail_path, can_delete))
        }

        // ── Purchase History Section (placeholder) ──
        div class="detail-card" style="margin-top:var(--space-5)" {
            div class="detail-card-title" { "采购历史" }
            div class="empty-state" { "暂无采购记录" }
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
            "bank-account-create-modal",
            "添加银行账户",
            "保存",
            "create-bank-account-form",
            &bank_account_create_path.to_string(),
            html! {
                div class="form-grid" {
                    div class="form-field" {
                        label { "开户银行 *" }
                        input type="text" name="bank_name" required placeholder="请输入开户银行";
                    }
                    div class="form-field" {
                        label { "账户名称 *" }
                        input type="text" name="account_name" required placeholder="请输入账户名称";
                    }
                    div class="form-field field-full" {
                        label { "银行账号 *" }
                        input type="text" name="account_number" required placeholder="请输入银行账号";
                    }
                    div class="form-field" {
                        label class="checkbox-label" {
                            input type="checkbox" name="is_default" value="true";
                            "默认账户"
                        }
                    }
                }
            },
        ))
        }
    }
}

fn contacts_card(contacts: &[SupplierContact], detail_path: &SupplierDetailPath, can_delete: bool) -> Markup {
    html! {
        div class="detail-card" id="contacts-card"
            hx-get=(detail_path.to_string())
            hx-select="#contacts-card"
            hx-target="this"
            hx-swap="outerHTML"
            hx-trigger="contactChanged from:body" {
            div class="detail-card-title" {
                span { "联系人" }
                button class="btn btn-sm btn-primary"
                    _="on click add .is-open to #contact-create-modal" {
                    (icon::plus_icon("w-3.5 h-3.5"))
                    "添加联系人"
                }
            }
            @if contacts.is_empty() {
                div class="empty-state" { "暂无联系人" }
            } @else {
                table class="data-table compact" {
                    thead {
                        tr {
                            th { "姓名" }
                            th { "职位" }
                            th { "电话" }
                            th { "邮箱" }
                            th style="width:60px" { "标记" }
                            th style="width:40px" {}
                        }
                    }
                    tbody {
                        @for c in contacts {
                            (contact_row(c, detail_path, can_delete))
                        }
                    }
                }
            }
        }
    }
}

fn bank_accounts_card(bank_accounts: &[SupplierBankAccount], detail_path: &SupplierDetailPath, can_delete: bool) -> Markup {
    html! {
        div class="detail-card" id="bank-accounts-card"
            hx-get=(detail_path.to_string())
            hx-select="#bank-accounts-card"
            hx-target="this"
            hx-swap="outerHTML"
            hx-trigger="bankAccountChanged from:body" {
            div class="detail-card-title" {
                span { "银行账户" }
                button class="btn btn-sm btn-primary"
                    _="on click add .is-open to #bank-account-create-modal" {
                    (icon::plus_icon("w-3.5 h-3.5"))
                    "添加账户"
                }
            }
            @if bank_accounts.is_empty() {
                div class="empty-state" { "暂无银行账户" }
            } @else {
                table class="data-table compact" {
                    thead {
                        tr {
                            th { "开户银行" }
                            th { "账户名称" }
                            th { "银行账号" }
                            th style="width:60px" { "标记" }
                            th style="width:40px" {}
                        }
                    }
                    tbody {
                        @for ba in bank_accounts {
                            (bank_account_row(ba, detail_path, can_delete))
                        }
                    }
                }
            }
        }
    }
}

fn contact_row(contact: &SupplierContact, detail_path: &SupplierDetailPath, can_delete: bool) -> Markup {
    let delete_path = SupplierDeleteContactPath {
        sid: detail_path.id,
        contact_id: contact.id,
    };
    let confirm_msg = format!("删除后无法恢复，确定要删除联系人 {} 吗？", contact.name);

    html! {
        tr {
            td { (contact.name) }
            td { (contact.position.as_deref().unwrap_or("—")) }
            td { (contact.phone.as_deref().unwrap_or("—")) }
            td { (contact.email.as_deref().unwrap_or("—")) }
            td {
                @if contact.is_primary {
                    span class="tag-chip tag-key" { "主要" }
                }
            }
            td {
                @if can_delete {
                    button type="button" class="row-action-btn text-danger" title="删除"
                        hx-post=(delete_path)
                        hx-confirm=(confirm_msg)
                        hx-swap="none" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn bank_account_row(account: &SupplierBankAccount, detail_path: &SupplierDetailPath, can_delete: bool) -> Markup {
    let delete_path = SupplierDeleteBankAccountPath {
        sid: detail_path.id,
        account_id: account.id,
    };
    let confirm_msg = format!("删除后无法恢复，确定要删除银行账户 {} 吗？", account.bank_name);

    html! {
        tr {
            td { (account.bank_name) }
            td { (account.account_name) }
            td class="mono" { (account.account_number) }
            td {
                @if account.is_default {
                    span class="tag-chip tag-key" { "默认" }
                }
            }
            td {
                @if can_delete {
                    button type="button" class="row-action-btn text-danger" title="删除"
                        hx-post=(delete_path)
                        hx-confirm=(confirm_msg)
                        hx-swap="none" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}
