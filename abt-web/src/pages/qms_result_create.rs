use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::qms::enums::InspectionSourceType;
use abt_core::qms::inspection_result::model::{CheckResult, CreateInspectionResultReq};
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::inspection_specification::model::InspectionSpecFilter;
use abt_core::qms::inspection_specification::InspectionSpecificationService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{ResultCreatePath, ResultListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct ResultCreateForm {
    pub spec_id: i64,
    pub source_type: i16,
    pub source_id: i64,
    pub batch_no: String,
    pub sample_qty: String,
    pub result: i16,
    pub qualified_qty: String,
    pub unqualified_qty: String,
    pub inspector_id: Option<i64>,
    pub inspection_date: String,
    pub check_results_json: String,
}

// ── Handlers ──

#[require_permission("QMS", "create")]
pub async fn get_create(
    _path: ResultCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let spec_svc = state.inspection_specification_service();
    let filter = InspectionSpecFilter {
        status: Some(abt_core::qms::enums::SpecStatus::Active),
        ..Default::default()
    };
    let specs = spec_svc
        .list(&service_ctx, &mut conn, filter, PageParams { page: 1, page_size: 200 })
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    let user_svc = state.user_service();
    let users = user_svc
        .list_users(&service_ctx, &mut conn, 1, 200)
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    let content = result_create_page(&specs, &users);
    let page_html = admin_page(
        is_htmx,
        "记录检验结果",
        &claims,
        "quality",
        ResultCreatePath::PATH,
        "质量管理",
        Some(ResultListPath::PATH),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("QMS", "create")]
pub async fn create(
    _path: ResultCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ResultCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let source_type = InspectionSourceType::from_i16(form.source_type).ok_or_else(|| {
        abt_core::shared::types::DomainError::Validation("无效来源类型".into())
    })?;

    let sample_qty: rust_decimal::Decimal = form.sample_qty.parse().unwrap_or_default();
    let qualified_qty: rust_decimal::Decimal = form.qualified_qty.parse().unwrap_or_default();
    let unqualified_qty: rust_decimal::Decimal = form.unqualified_qty.parse().unwrap_or_default();

    let req = CreateInspectionResultReq {
        spec_id: form.spec_id,
        source_type,
        source_id: form.source_id,
        batch_no: form.batch_no,
        sample_qty,
    };

    let svc = state.inspection_result_service();
    let id = svc.create(&service_ctx, &mut conn, req).await?;

    // Parse check results JSON and record result if provided
    let result_type = abt_core::qms::enums::InspectionResultType::from_i16(form.result);
    if let Some(result_val) = result_type {
        let check_results: Vec<CheckResult> = if form.check_results_json.is_empty() {
            vec![]
        } else {
            serde_json::from_str(&form.check_results_json).unwrap_or_default()
        };

        let inspection_date = chrono::NaiveDate::parse_from_str(&form.inspection_date, "%Y-%m-%d")
            .ok();

        if let Some(inspection_date) = inspection_date {
            let record_req = abt_core::qms::inspection_result::model::RecordInspectionResultReq {
                result: result_val,
                qualified_qty,
                unqualified_qty,
                check_results,
                inspector_id: form.inspector_id.unwrap_or(1),
                inspection_date,
            };
            let _gate = svc.record_result(&service_ctx, &mut conn, id, record_req).await?;
        }
    }

    Ok(
        axum::response::Response::builder()
            .header("HX-Redirect", ResultListPath::PATH)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
}

// ── Page rendering ──

fn result_create_page(
    specs: &[abt_core::qms::inspection_specification::model::InspectionSpecification],
    users: &[abt_core::shared::identity::model::User],
) -> Markup {
    html! {
        div {
            // ── Back link ──
            a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ResultListPath::PATH)) {
                (icon::arrow_left_icon(""))
                " 返回检验结果列表"
            }

            // ── Page header ──
            div class="flex items-center justify-between mb-6" {
                div class="flex items-center justify-between mb-6-left" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "记录检验结果" }
                }
                div class="flex items-center justify-between mb-6-right" {
                    span class="text-sm text-muted" { "自动保存草稿" }
                }
            }

            form id="result-form" hx-post=(ResultCreatePath::PATH) hx-swap="none" {

                // ── Section 1: 检验信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
                        (icon::file_text_icon(""))
                        " 检验信息"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验规格 " span style="color:var(--danger)" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="spec_id" required {
                                option value="" disabled selected { "请选择检验规格" }
                                @for spec in specs {
                                    option value=(spec.id) {
                                        (spec.doc_number)
                                        " - "
                                        @match spec.inspection_type {
                                            abt_core::qms::enums::InspectionType::Iqc => "IQC",
                                            abt_core::qms::enums::InspectionType::Ipqc => "IPQC",
                                            abt_core::qms::enums::InspectionType::Fqc => "FQC",
                                            abt_core::qms::enums::InspectionType::Oqc => "OQC",
                                        }
                                    }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源类型 " span style="color:var(--danger)" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="source_type" required {
                                option value="" disabled selected { "请选择来源类型" }
                                option value="1" { "来料通知" }
                                option value="2" { "工单工序" }
                                option value="3" { "发货单" }
                                option value="4" { "委外单" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源单号 " span style="color:var(--danger)" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="source_id" required placeholder="请输入来源单号";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "批次号 " span style="color:var(--danger)" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" required placeholder="请输入批次号";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "抽样数量 " span style="color:var(--danger)" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="sample_qty" step="any" required placeholder="请输入抽样数量";
                        }
                    }
                }

                // ── Section 2: 检验结果 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
                        (icon::check_circle_icon(""))
                        " 检验结果"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验结论 " span style="color:var(--danger)" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="result" required {
                                option value="" disabled selected { "请选择检验结论" }
                                option value="1" { "合格" }
                                option value="2" { "不合格" }
                                option value="3" { "让步接收" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "合格数量" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="qualified_qty" step="any" min="0" placeholder="0";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "不合格数量" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="unqualified_qty" step="any" min="0" placeholder="0";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验员" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="inspector_id" {
                                option value="" disabled selected { "请选择检验员" }
                                @for user in users {
                                    @if user.is_active {
                                        option value=(user.user_id) {
                                            (user.display_name.as_deref().unwrap_or(&user.username))
                                        }
                                    }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验日期" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="inspection_date";
                        }
                    }
                }

                // ── Section 3: 检验项目明细 ──
                div class="form-section" style="padding:0;overflow:hidden" {
                    div style="padding:var(--space-6) var(--space-6) var(--space-4)" {
                        div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" style="border-bottom:none;padding-bottom:0;margin-bottom:0" {
                            (icon::clipboard_list_icon(""))
                            " 检验项目明细"
                        }
                    }
                    div style="overflow-x:auto" {
                        table class="w-full border-collapse" {
                            thead {
                                tr {
                                    th style="width:50px;text-align:center" { "序号" }
                                    th style="min-width:140px" { "检验项目" }
                                    th style="min-width:140px" { "检验标准" }
                                    th style="min-width:120px" { "实测值" }
                                    th style="width:110px;text-align:center" { "是否合格" }
                                    th style="min-width:120px" { "备注" }
                                }
                            }
                            tbody id="check-items-body" {
                                // 5 pre-filled example rows
                                @for i in 1..=5 {
                                    tr {
                                        td class="text-muted text-xs text-center" { (i) }
                                        td {
                                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text"
                                                name={"item_" (i)}
                                                placeholder="检验项目";
                                        }
                                        td {
                                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text"
                                                name={"standard_" (i)}
                                                placeholder="检验标准";
                                        }
                                        td {
                                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text"
                                                name={"measured_" (i)}
                                                placeholder="实测值";
                                        }
                                        td style="text-align:center" {
                                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name={"pass_" (i)} {
                                                option value="" { "—" }
                                                option value="1" { "✓ 合格" }
                                                option value="0" { "✗ 不合格" }
                                            }
                                        }
                                        td {
                                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text"
                                                name={"remark_" (i)}
                                                placeholder="备注";
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Hidden field for check results JSON ──
                input type="hidden" name="check_results_json" id="check-results-json" value="";

                // ── Action bar ──
                div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", ResultListPath::PATH)) { "取消" }
                    button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" id="btn-save-draft" { "保存草稿" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "提交检验结果" }
                }
            }

            // ── Inline script: collect check items before submit ──
            script {
                (maud::PreEscaped(r#"
document.getElementById('result-form').addEventListener('htmx:beforeRequest', function(e) {
    var rows = document.querySelectorAll('#check-items-body tr');
    var items = [];
    rows.forEach(function(row, idx) {
        var item = row.querySelector('input[name="item_' + (idx+1) + '"]');
        var standard = row.querySelector('input[name="standard_' + (idx+1) + '"]');
        var measured = row.querySelector('input[name="measured_' + (idx+1) + '"]');
        var pass = row.querySelector('select[name="pass_' + (idx+1) + '"]');
        var remark = row.querySelector('input[name="remark_' + (idx+1) + '"]');
        if (item && item.value) {
            items.push({
                item: item.value,
                standard: standard ? standard.value : '',
                measured: measured ? measured.value : '',
                pass: pass ? pass.value === '1' : false,
                remark: remark ? remark.value : null
            });
        }
    });
    document.getElementById('check-results-json').value = JSON.stringify(items);
});
"#))
            }
        }
    }
}
