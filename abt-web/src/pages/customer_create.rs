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
use crate::routes::customer::{CreateCustomerPath, CustomerDetailPath, CustomerListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct CreateCustomerForm {
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
 // 操作
 action: Option<String>,
}

// ── Handlers ──

#[require_permission("CUSTOMER", "create")]
pub async fn get_customer_create(
 _path: CreateCustomerPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, state, service_ctx, mut conn, .. } = ctx;

 // 获取活跃用户列表（负责业务员下拉）
 let user_svc = state.user_service();
 let users = user_svc.list_users_with_roles(&service_ctx, &mut conn)
 .await
 .unwrap_or_default()
 .into_iter()
 .filter(|u| u.user.is_active)
 .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
 .collect::<Vec<_>>();

 let content = customer_create_page(&users);
 let page_html = admin_page(
 is_htmx,
 "新建客户",
 &claims,
 "sales",
 CreateCustomerPath::PATH,
 "销售管理",
 Some(CustomerListPath::PATH),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("CUSTOMER", "create")]
pub async fn post_customer_create(
 _path: CreateCustomerPath,
 ctx: RequestContext,
 Form(form): Form<CreateCustomerForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.customer_service();

 let category = form
 .category
 .and_then(CustomerCategory::from_i16)
 .unwrap_or(CustomerCategory::DirectCustomer);

 let credit_limit = form
 .credit_limit
 .filter(|s| !s.is_empty())
 .and_then(|s| s.parse::<rust_decimal::Decimal>().ok());

 let req = CreateCustomerReq {
 customer_name: form.customer_name,
 short_name: form.short_name.filter(|s| !s.is_empty()),
 category,
 industry: form.industry.filter(|s| !s.is_empty()),
 customer_level: form.customer_level.and_then(CustomerLevel::from_i16),
 region: form.region.filter(|s| !s.is_empty()),
 tax_number: form.tax_number.filter(|s| !s.is_empty()),
 invoice_title: form.invoice_title.filter(|s| !s.is_empty()),
 credit_limit,
 payment_terms: form.payment_terms.filter(|s| !s.is_empty()),
 currency: form.currency.filter(|s| !s.is_empty()),
 receivable_account: None,
 owner_id: form.owner_id.filter(|s| !s.is_empty()).and_then(|s| s.parse::<i64>().ok()),
 source: form.source.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 };

 let id = svc.create(&service_ctx, &mut conn, req).await?;

 // 如果提供了联系人信息，自动创建主要联系人
 if let Some(contact_name) = form.contact_name.filter(|s| !s.is_empty()) {
 let contact_req = CreateContactReq {
 contact_name,
 phone: form.contact_phone.filter(|s| !s.is_empty()),
 email: form.contact_email.filter(|s| !s.is_empty()),
 position: form.contact_position.filter(|s| !s.is_empty()),
 fax: form.contact_fax.filter(|s| !s.is_empty()),
 fixed_phone: form.contact_fixed_phone.filter(|s| !s.is_empty()),
 is_primary: true,
 };
 svc.add_contact(&service_ctx, &mut conn, id, contact_req).await?;
 }

 let redirect = match form.action.as_deref() {
 Some("continue") => CreateCustomerPath.to_string(),
 _ => CustomerDetailPath { id }.to_string(),
 };
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Rendering ──

fn customer_create_page(users: &[(i64, String)]) -> Markup {
 html! {
    div {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(format!("{}?restore=true", CustomerListPath::PATH))
            { (icon::arrow_left_icon("w-4 h-4")) "返回客户列表" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建客户" }
        }

        form hx-post=(CreateCustomerPath::PATH) hx-swap="none" {
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
                        input type="text" name="customer_name" required placeholder="请输入客户全称" {}
                    }
                    div class="form-field" {
                        label { "客户简称" }
                        input type="text" name="short_name" placeholder="请输入客户简称" {}
                    }
                    div class="form-field" {
                        label { "客户编码" }
                        input type="text" value="自动生成" readonly class="bg-surface text-muted" {}
                    }
                    div class="form-field" {
                        label { "所属行业" }
                        select name="industry" {
                            option value="" { "请选择行业" }
                            option value="电子制造" { "电子制造" }
                            option value="精密模具" { "精密模具" }
                            option value="光电科技" { "光电科技" }
                            option value="半导体" { "半导体" }
                            option value="陶瓷机械" { "陶瓷机械" }
                            option value="自动化技术" { "自动化技术" }
                            option value="新材料" { "新材料" }
                            option value="其他" { "其他" }
                        }
                    }
                    div class="form-field" {
                        label { "客户等级" }
                        select name="customer_level" {
                            option value="1" selected { "普通客户" }
                            option value="2" { "关键客户" }
                            option value="3" { "潜在客户" }
                        }
                    }
                    div class="form-field" {
                        label { "所在区域" }
                        select name="region" {
                            option value="" { "请选择区域" }
                            option value="华南地区" { "华南地区" }
                            option value="华东地区" { "华东地区" }
                            option value="华北地区" { "华北地区" }
                            option value="西南地区" { "西南地区" }
                            option value="其他" { "其他" }
                        }
                    }
                }
            }
            // ── Section 2: 联系信息 ──
            div class="data-card" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "联系信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "联系人 "
                            span class="text-danger" { "*" }
                        }
                        input type="text" name="contact_name" required placeholder="请输入联系人姓名" {}
                    }
                    div class="form-field" {
                        label { "职位" }
                        input type="text" name="contact_position" placeholder="例如：采购经理" {}
                    }
                    div class="form-field" {
                        label {
                            "手机号码 "
                            span class="text-danger" { "*" }
                        }
                        input type="tel" name="contact_phone" required placeholder="请输入手机号码" {}
                    }
                    div class="form-field" {
                        label { "固定电话" }
                        input
                            type="tel"
                            name="contact_fixed_phone"
                            placeholder="例如：0755-8866-2201" {}
                    }
                    div class="form-field" {
                        label { "邮箱" }
                        input type="email" name="contact_email" placeholder="请输入邮箱地址" {}
                    }
                    div class="form-field" {
                        label { "传真" }
                        input type="tel" name="contact_fax" placeholder="请输入传真号码" {}
                    }
                    div class="form-field field-full" {
                        label { "详细地址" }
                        input type="text" name="contact_address" placeholder="请输入完整地址" {}
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
                        input type="number" step="any" name="credit_limit" placeholder="请输入信用额度" {}
                    }
                    div class="form-field" {
                        label { "付款条款" }
                        select name="payment_terms" {
                            option value="月结30天" selected { "月结 30 天" }
                            option value="月结60天" { "月结 60 天" }
                            option value="月结90天" { "月结 90 天" }
                            option value="预付款" { "预付款" }
                            option value="货到付款" { "货到付款" }
                        }
                    }
                    div class="form-field" {
                        label { "币种" }
                        select name="currency" {
                            option value="CNY" selected { "CNY (人民币)" }
                            option value="USD" { "USD (美元)" }
                            option value="EUR" { "EUR (欧元)" }
                        }
                    }
                    div class="form-field" {
                        label { "税号" }
                        input type="text" name="tax_number" placeholder="请输入纳税人识别号" {}
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
                            option value="" { "请选择业务员" }
                            @for (uid, uname) in users {
                                option value=(uid) { (uname) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "客户来源" }
                        select name="source" {
                            option value="" { "请选择来源" }
                            option value="自主开发" { "自主开发" }
                            option value="展会获客" { "展会获客" }
                            option value="老客户转介" { "老客户转介" }
                            option value="网络推广" { "网络推广" }
                        }
                    }
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" placeholder="请输入备注信息…" rows="4" {}
                    }
                }
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", CustomerListPath::PATH))
                { "取消" }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    name="action"
                    value="continue"
                { "保存并继续" }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { "保存客户" }
            }
        }
    }
}
}
