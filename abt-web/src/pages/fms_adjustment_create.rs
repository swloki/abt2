use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::adjustment::enums::AdjustmentDirection;
use abt_core::fms::adjustment::model::CreateAdjustmentReq;
use abt_core::fms::adjustment::AdjustmentService;
use abt_core::fms::ar_ap::ArApService;
use abt_core::fms::enums::CounterpartyType;

use crate::components::{entity_picker, icon};
use crate::components::entity_picker::EntityPickerConfig;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{
    AdjustmentBalancePath, ApAdjustmentCreatePath, ApAdjustmentListPath, ArAdjustmentCreatePath,
    ArAdjustmentListPath, JournalSearchCpPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct AdjustmentCreateForm {
    pub party_id: i64,
    pub direction: i16,
    pub amount: String,
    pub adjustment_date: String,
    pub int_order_no: Option<String>,
    pub ext_order_no: Option<String>,
    pub description: String,
}

// ── 余额查询端点（选往来方后 htmx 加载，只读参考）──

#[derive(Debug, Deserialize)]
pub struct BalanceQuery {
    pub party_type: i16,
    pub party_id: i64,
}

#[require_permission("FMS", "read")]
pub async fn get_balance(
    _path: AdjustmentBalancePath,
    ctx: RequestContext,
    Query(q): Query<BalanceQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let party_type = CounterpartyType::from_i16(q.party_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;

    let balance = state
        .ar_ap_service()
        .get_party_balance(&service_ctx, &mut conn, party_type, q.party_id)
        .await
        .ok();

    let (amount, label) = match (&balance, party_type) {
        (Some(b), CounterpartyType::Customer) => (b.total_ar, "当前应收余额"),
        (Some(b), _) => (b.total_ap, "当前应付余额"),
        (None, _) => {
            return Ok(Html(
                "<div class='text-sm text-muted'>无法获取余额</div>".to_string(),
            ));
        }
    };

    Ok(Html(format!(
        "<div class='text-2xl font-bold font-mono tabular-nums text-fg'>¥{amount:.2}</div>\
         <div class='text-xs text-muted mt-1'>{label}</div>"
    )))
}

// ── 创建页 GET ──

#[require_permission("FMS", "read")]
pub async fn get_ar_create(
    _path: ArAdjustmentCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    render_create_page(ctx, CounterpartyType::Customer).await
}

#[require_permission("FMS", "read")]
pub async fn get_ap_create(
    _path: ApAdjustmentCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    render_create_page(ctx, CounterpartyType::Supplier).await
}

async fn render_create_page(
    ctx: RequestContext,
    party_type: CounterpartyType,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;

    let (title, list_path, create_path, cp_label, picker_title, placeholder, balance_label) =
        match party_type {
            CounterpartyType::Customer => (
                "新建应收调整",
                ArAdjustmentListPath::PATH,
                ArAdjustmentCreatePath::PATH,
                "客户",
                "选择客户",
                "搜索选择客户…",
                "应收金额",
            ),
            _ => (
                "新建应付调整",
                ApAdjustmentListPath::PATH,
                ApAdjustmentCreatePath::PATH,
                "供应商",
                "选择供应商",
                "搜索选择供应商…",
                "应付金额",
            ),
        };

    let content =
        adjustment_create_page(party_type, cp_label, picker_title, placeholder, balance_label);
    let page_html = admin_page(
        is_htmx,
        title,
        &claims,
        "finance",
        create_path,
        "财务管理",
        Some(list_path),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

fn adjustment_create_page(
    party_type: CounterpartyType,
    cp_label: &str,
    picker_title: &str,
    placeholder: &str,
    balance_label: &str,
) -> Markup {
    let (back_list, create_path) = match party_type {
        CounterpartyType::Customer => (ArAdjustmentListPath::PATH, ArAdjustmentCreatePath::PATH),
        _ => (ApAdjustmentListPath::PATH, ApAdjustmentCreatePath::PATH),
    };
    let cp_type_val: i16 = match party_type {
        CounterpartyType::Customer => 1,
        _ => 2,
    };

    let cp_picker = EntityPickerConfig {
        modal_id: "cp-picker",
        title: picker_title,
        search_label: "关键词",
        search_placeholder: "搜索名称或编码…",
        search_path: JournalSearchCpPath::PATH,
        search_param: "q",
        target_id: "party-id",
        display_id: "party-display",
        event_name: "counterpartySelected",
        extra_include: Some("#cp-type-hidden"),
    };

    html! {
        div {
            a href=(format!("{}?restore=true", back_list))
                class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg mb-4"
            { (icon::chevron_left_icon("w-4 h-4")) "返回列表" }
            h1 class="text-xl font-bold text-fg tracking-tight mb-6" {
                (match party_type { CounterpartyType::Customer => "新建应收调整", _ => "新建应付调整" })
            }

            form id="adjustment-create-form" hx-post=(create_path) hx-swap="none" {
                input type="hidden" id="cp-type-hidden" name="counterparty_type" value=(cp_type_val);

                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                    { (icon::clipboard_document_icon("w-4 h-4")) " 基本信息" }

                    div class="grid grid-cols-2 gap-4 gap-x-6" {
                        // 往来方选择器
                        ({
                            entity_picker::entity_picker_field(
                                "party_id", "party-id", "party-display", "cp-picker",
                                &format!("{cp_label} "), true, placeholder,
                            )
                        })
                        // 当前余额（只读，选往来方后 htmx 加载）
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { (balance_label) }
                            div id="balance-display"
                                class="px-3 py-2 border border-border rounded-sm bg-surface min-h-[42px] flex flex-col justify-center"
                                hx-get=(AdjustmentBalancePath::PATH)
                                hx-trigger="counterpartySelected from:body"
                                hx-target="this"
                                hx-swap="innerHTML"
                                hx-include="#party-id"
                                hx-vals=(format!("{{\"party_type\":\"{cp_type_val}\"}}"))
                            {
                                div class="text-sm text-muted" { "请先选择" (cp_label) }
                            }
                        }
                        // 调整方向
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "调整方向 " span class="text-danger" { "*" }
                            }
                            select name="direction" required
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                            {
                                option value="" disabled selected { "请选择" }
                                option value="1" { "增加" }
                                option value="2" { "减少" }
                            }
                        }
                        // 调整金额(含税)
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "调整金额(含税) " span class="text-danger" { "*" }
                            }
                            input type="number" step="any" name="amount" required
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono text-right"
                                placeholder="0.00";
                        }
                        // 调整日期
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "调整日期 " span class="text-danger" { "*" }
                            }
                            input type="date" name="adjustment_date" required
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
                        }
                        // 调整单号（自动生成）
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "调整单号" }
                            input type="text" disabled value="系统自动生成"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted cursor-not-allowed";
                        }
                        // 内部订单号
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "内部订单号" }
                            input type="text" name="int_order_no"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                                placeholder="可选";
                        }
                        // 外部订单号
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                (cp_label) "订单号"
                            }
                            input type="text" name="ext_order_no"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                                placeholder="可选";
                        }
                    }
                    // 简要说明
                    div class="form-field mt-4" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "简要说明" }
                        textarea name="description" rows="2"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y"
                            placeholder="如：坏账核销 / 折扣 / 抹零 / 错误更正…" {}
                    }
                }
            }
        }
        (entity_picker::entity_picker_modal(&cp_picker))
        // Action bar
        div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
        {
            a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm font-medium cursor-pointer"
                href=(format!("{}?restore=true", back_list))
            { "取消" }
            button type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on text-sm font-medium cursor-pointer"
                _="on click trigger submit on #adjustment-create-form"
            { (icon::check_circle_icon("w-4 h-4")) "提交" }
        }
    }
}

// ── 创建 POST ──

#[require_permission("FMS", "create")]
pub async fn create_ar(
    _path: ArAdjustmentCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<AdjustmentCreateForm>,
) -> Result<axum::response::Response> {
    do_create(ctx, form, CounterpartyType::Customer, ArAdjustmentListPath::PATH).await
}

#[require_permission("FMS", "create")]
pub async fn create_ap(
    _path: ApAdjustmentCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<AdjustmentCreateForm>,
) -> Result<axum::response::Response> {
    do_create(ctx, form, CounterpartyType::Supplier, ApAdjustmentListPath::PATH).await
}

async fn do_create(
    ctx: RequestContext,
    form: AdjustmentCreateForm,
    party_type: CounterpartyType,
    redirect_path: &'static str,
) -> Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    if form.party_id <= 0 {
        return Err(abt_core::shared::types::DomainError::Validation("请选择往来方".into()).into());
    }
    let direction = AdjustmentDirection::from_i16(form.direction)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效调整方向".into()))?;
    let amount = rust_decimal::Decimal::from_str_exact(&form.amount)
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效金额".into()))?;
    let adjustment_date = chrono::NaiveDate::parse_from_str(&form.adjustment_date, "%Y-%m-%d")
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效调整日期".into()))?;
    let period = form
        .adjustment_date
        .get(..7)
        .unwrap_or(&form.adjustment_date)
        .to_string();

    let req = CreateAdjustmentReq {
        party_type,
        party_id: form.party_id,
        direction,
        amount,
        adjustment_date,
        period,
        int_order_no: form.int_order_no.filter(|s| !s.is_empty()),
        ext_order_no: form.ext_order_no.filter(|s| !s.is_empty()),
        description: form.description,
    };

    let svc = state.adjustment_service();
    svc.create_adjustment(&service_ctx, &mut conn, req).await?;

    Ok(axum::response::Response::builder()
        .header("HX-Redirect", redirect_path)
        .body(axum::body::Body::empty())
        .unwrap())
}
