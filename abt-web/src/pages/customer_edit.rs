use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::*;
use abt_core::master_data::customer::CustomerService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::customer::{
 CustomerDetailPath, CustomerListPath, EditCustomerPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct EditCustomerForm {
 // 基本信息
 customer_name: String,
 short_name: Option<String>,
 category: Option<i16>,
 industry: Option<String>,
 customer_level: Option<i16>,
 region: Option<String>,
 // 联系信息
 contact_name: Option<String>,
 contact_position: Option<String>,
 contact_phone: Option<String>,
 contact_fixed_phone: Option<String>,
 contact_email: Option<String>,
 contact_fax: Option<String>,
 contact_address: Option<String>,
 // 财务信息
 credit_limit: Option<String>,
 payment_terms: Option<String>,
 currency: Option<String>,
 tax_number: Option<String>,
 invoice_title: Option<String>,
 // 其他信息
 owner_id: Option<String>,
 source: Option<String>,
 remark: Option<String>,
 // 隐藏：主联系人 ID（如果存在则更新，否则新建）
 primary_contact_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_edit(
 path: EditCustomerPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 claims,
 state,
 service_ctx,
 mut conn,
 ..
 } = ctx;

 let svc = state.customer_service();
 let customer = svc.get(&service_ctx, &mut conn, path.id).await?;
 let contacts = svc
 .list_contacts(&service_ctx, &mut conn, path.id)
 .await
 .unwrap_or_default();
 let primary_contact = contacts.iter().find(|c| c.is_primary);

 // 获取活跃用户列表（负责业务员下拉）
 let user_svc = state.user_service();
 let users = user_svc
 .list_users_with_roles(&service_ctx, &mut conn)
 .await
 .unwrap_or_default()
 .into_iter()
 .filter(|u| u.user.is_active)
 .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
 .collect::<Vec<_>>();

 let content = customer_edit_page(&customer, primary_contact, &users);
 let page_html = admin_page(
 is_htmx,
 "编辑客户",
 &claims,
 "sales",
 EditCustomerPath::PATH,
 "销售管理",
 Some(CustomerListPath::PATH),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("CUSTOMER", "update")]
pub async fn post_customer_edit(
 path: EditCustomerPath,
 ctx: RequestContext,
 Form(form): Form<EditCustomerForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 state,
 service_ctx,
 ..
 } = ctx;
 let svc = state.customer_service();

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let category = form
 .category
 .and_then(CustomerCategory::from_i16)
 .unwrap_or(CustomerCategory::DirectCustomer);

 let credit_limit = form
 .credit_limit
 .filter(|s| !s.is_empty())
 .and_then(|s| s.parse::<rust_decimal::Decimal>().ok());

 let req = UpdateCustomerReq {
 customer_name: Some(form.customer_name),
 short_name: form.short_name.filter(|s| !s.is_empty()),
 category: Some(category),
 industry: form.industry.filter(|s| !s.is_empty()),
 customer_level: form.customer_level.and_then(CustomerLevel::from_i16),
 region: form.region.filter(|s| !s.is_empty()),
 status: None,
 tax_number: form.tax_number.filter(|s| !s.is_empty()),
 invoice_title: form.invoice_title.filter(|s| !s.is_empty()),
 credit_limit,
 payment_terms: form.payment_terms.filter(|s| !s.is_empty()),
 currency: form.currency.filter(|s| !s.is_empty()),
 receivable_account: None,
 source: form.source.filter(|s| !s.is_empty()),
 owner_id: form
 .owner_id
 .filter(|s| !s.is_empty())
 .and_then(|s| s.parse::<i64>().ok()),
 remark: form.remark.filter(|s| !s.is_empty()),
 };

 svc.update(&service_ctx, &mut tx, path.id, req)
 .await?;

 // 更新或创建主联系人
 let contact_name = form.contact_name.filter(|s| !s.is_empty());
 if let Some(name) = contact_name {
 if let Some(cid) = form.primary_contact_id {
 // 更新现有联系人
 let contact_req = UpdateContactReq {
 contact_name: Some(name),
 phone: form.contact_phone.filter(|s| !s.is_empty()),
 email: form.contact_email.filter(|s| !s.is_empty()),
 position: form.contact_position.filter(|s| !s.is_empty()),
 fax: form.contact_fax.filter(|s| !s.is_empty()),
 fixed_phone: form.contact_fixed_phone.filter(|s| !s.is_empty()),
 is_primary: None,
 };
 svc.update_contact(&service_ctx, &mut tx, path.id, cid, contact_req)
 .await?;
 } else {
 // 新建联系人
 let contact_req = CreateContactReq {
 contact_name: name,
 phone: form.contact_phone.filter(|s| !s.is_empty()),
 email: form.contact_email.filter(|s| !s.is_empty()),
 position: form.contact_position.filter(|s| !s.is_empty()),
 fax: form.contact_fax.filter(|s| !s.is_empty()),
 fixed_phone: form.contact_fixed_phone.filter(|s| !s.is_empty()),
 is_primary: true,
 };
 svc.add_contact(&service_ctx, &mut tx, path.id, contact_req)
 .await?;
 }
 }

 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = CustomerDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Rendering ──

fn customer_edit_page(
 customer: &Customer,
 primary_contact: Option<&CustomerContact>,
 users: &[(i64, String)],
) -> Markup {
 let detail_path = CustomerDetailPath { id: customer.id }.to_string();
 let c = customer;
 let pc = primary_contact;

 // 预填值辅助
 let industry_val = c.industry.as_deref().unwrap_or("");
 let level_val = c
 .customer_level
 .map(|l| l.as_i16().to_string())
 .unwrap_or_default();
 let region_val = c.region.as_deref().unwrap_or("");
 let credit_val = c
 .credit_limit
 .map(|l| format!("{:.2}", l))
 .unwrap_or_default();
 let payment_val = c.payment_terms.as_deref().unwrap_or("");
 let currency_val = c.currency.as_deref().unwrap_or("CNY");
 let owner_val = c.owner_id.map(|id| id.to_string()).unwrap_or_default();
 let source_val = c.source.as_deref().unwrap_or("");

 html! {
    div {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(detail_path)
            { (icon::arrow_left_icon("w-4 h-4")) "返回客户详情" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "编辑客户" }
        }

        form hx-post=(EditCustomerPath { id: c.id }.to_string()) hx-swap="none" {
            // ── Section 1: 基本信息 ──
            div class="data-card" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "客户名称 "
                            span class="text-danger" { "*" }
                        }
                        input
                            type="text"
                            name="customer_name"
                            required
                            placeholder="请输入客户全称"
                            value=(c.name);
                    }
                    div class="form-field" {
                        label { "客户简称" }
                        input
                            type="text"
                            name="short_name"
                            placeholder="请输入客户简称"
                            value=(c.short_name.as_deref().unwrap_or(""));
                    }
                    div class="form-field" {
                        label { "客户编码" }
                        input type="text" value=(c.code) readonly class="bg-surface text-muted" {}
                        ;
                    }
                    div class="form-field" {
                        label { "所属行业" }
                        select name="industry" {
                            option value="" selected[industry_val.is_empty()] { "请选择行业" }
                            option value="电子制造" selected[industry_val == "电子制造"] {
                                "电子制造"
                            }
                            option value="精密模具" selected[industry_val == "精密模具"] {
                                "精密模具"
                            }
                            option value="光电科技" selected[industry_val == "光电科技"] {
                                "光电科技"
                            }
                            option value="半导体" selected[industry_val == "半导体"] { "半导体" }
                            option value="陶瓷机械" selected[industry_val == "陶瓷机械"] {
                                "陶瓷机械"
                            }
                            option value="自动化技术" selected[industry_val == "自动化技术"] {
                                "自动化技术"
                            }
                            option value="新材料" selected[industry_val == "新材料"] { "新材料" }
                            option value="其他" selected[industry_val == "其他"] { "其他" }
                        }
                    }
                    div class="form-field" {
                        label { "客户等级" }
                        select name="customer_level" {
                            option value="1" selected[level_val == "1"] { "普通客户" }
                            option value="2" selected[level_val == "2"] { "关键客户" }
                            option value="3" selected[level_val == "3"] { "潜在客户" }
                        }
                    }
                    div class="form-field" {
                        label { "所在区域" }
                        select name="region" {
                            option value="" selected[region_val.is_empty()] { "请选择区域" }
                            option value="华南地区" selected[region_val == "华南地区"] {
                                "华南地区"
                            }
                            option value="华东地区" selected[region_val == "华东地区"] {
                                "华东地区"
                            }
                            option value="华北地区" selected[region_val == "华北地区"] {
                                "华北地区"
                            }
                            option value="西南地区" selected[region_val == "西南地区"] {
                                "西南地区"
                            }
                            option value="其他" selected[region_val == "其他"] { "其他" }
                        }
                    }
                }
            }
            // ── Section 2: 联系信息 ──
            div class="data-card" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "联系信息" }
                // 隐藏字段：主联系人 ID
                @if let Some(contact) = pc {
                    input type="hidden" name="primary_contact_id" value=(contact.id);
                }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "联系人 "
                            span class="text-danger" { "*" }
                        }
                        input
                            type="text"
                            name="contact_name"
                            required
                            placeholder="请输入联系人姓名"
                            value=(pc.map(|c| c.name.as_str()).unwrap_or(""));
                    }
                    div class="form-field" {
                        label { "职位" }
                        input
                            type="text"
                            name="contact_position"
                            placeholder="例如：采购经理"
                            value=(pc.and_then(|c| c.position.as_deref()).unwrap_or(""));
                    }
                    div class="form-field" {
                        label {
                            "手机号码 "
                            span class="text-danger" { "*" }
                        }
                        input
                            type="tel"
                            name="contact_phone"
                            required
                            placeholder="请输入手机号码"
                            value=(pc.and_then(|c| c.phone.as_deref()).unwrap_or(""));
                    }
                    div class="form-field" {
                        label { "固定电话" }
                        input
                            type="tel"
                            name="contact_fixed_phone"
                            placeholder="例如：0755-8866-2201"
                            value=(pc.and_then(|c| c.fixed_phone.as_deref()).unwrap_or(""));
                    }
                    div class="form-field" {
                        label { "邮箱" }
                        input
                            type="email"
                            name="contact_email"
                            placeholder="请输入邮箱地址"
                            value=(pc.and_then(|c| c.email.as_deref()).unwrap_or(""));
                    }
                    div class="form-field" {
                        label { "传真" }
                        input
                            type="tel"
                            name="contact_fax"
                            placeholder="请输入传真号码"
                            value=(pc.and_then(|c| c.fax.as_deref()).unwrap_or(""));
                    }
                    div class="form-field field-full" {
                        label { "详细地址" }
                        input type="text" name="contact_address" placeholder="请输入完整地址" {}
                        ;
                    }
                }
            }
            // ── Section 3: 财务信息 ──
            div class="data-card" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "财务信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "信用额度 (元)" }
                        input
                            type="number"
                            step="any"
                            name="credit_limit"
                            placeholder="请输入信用额度"
                            value=(credit_val);
                    }
                    div class="form-field" {
                        label { "付款条款" }
                        select name="payment_terms" {
                            option value="月结30天" selected[payment_val == "月结30天"] {
                                "月结 30 天"
                            }
                            option value="月结60天" selected[payment_val == "月结60天"] {
                                "月结 60 天"
                            }
                            option value="月结90天" selected[payment_val == "月结90天"] {
                                "月结 90 天"
                            }
                            option value="预付款" selected[payment_val == "预付款"] { "预付款" }
                            option value="货到付款" selected[payment_val == "货到付款"] {
                                "货到付款"
                            }
                        }
                    }
                    div class="form-field" {
                        label { "币种" }
                        select name="currency" {
                            option value="CNY" selected[currency_val == "CNY"] { "CNY (人民币)" }
                            option value="USD" selected[currency_val == "USD"] { "USD (美元)" }
                            option value="EUR" selected[currency_val == "EUR"] { "EUR (欧元)" }
                        }
                    }
                    div class="form-field" {
                        label { "税号" }
                        input
                            type="text"
                            name="tax_number"
                            placeholder="请输入纳税人识别号"
                            value=(c.tax_number.as_deref().unwrap_or(""));
                    }
                }
            }
            // ── Section 4: 其他信息 ──
            div class="data-card" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "其他信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "负责业务员" }
                        select name="owner_id" {
                            option value="" selected[owner_val.is_empty()] { "请选择业务员" }
                            @for (uid, uname) in users {
                                option value=(uid) selected[owner_val == uid.to_string()] { (uname) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "客户来源" }
                        select name="source" {
                            option value="" selected[source_val.is_empty()] { "请选择来源" }
                            option value="自主开发" selected[source_val == "自主开发"] {
                                "自主开发"
                            }
                            option value="展会获客" selected[source_val == "展会获客"] {
                                "展会获客"
                            }
                            option value="老客户转介" selected[source_val == "老客户转介"] {
                                "老客户转介"
                            }
                            option value="网络推广" selected[source_val == "网络推广"] {
                                "网络推广"
                            }
                        }
                    }
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" placeholder="请输入备注信息…" rows="4" {
                            (c.remark)
                        }
                    }
                }
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                button
                    type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    onclick=(format!("location.href='{}'", detail_path))
                { "取消" }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { "保存修改" }
            }
        }
    }
}
}
