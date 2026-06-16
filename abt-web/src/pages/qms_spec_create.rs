use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::qms::inspection_specification::model::{
    CheckItem, CreateInspectionSpecificationReq, SamplePlan,
};
use abt_core::qms::inspection_specification::InspectionSpecificationService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::PageParams;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{SpecCreatePath, SpecListPath};
use crate::utils::RequestContext;
use crate::components::icon;
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct SpecCreateForm {
    pub product_id: i64,
    pub inspection_type: i16,
    // check_items sent as JSON hidden field
    pub check_items_json: String,
    pub sample_level: String,
    pub sample_aql: String,
    pub sample_mode: String,
}

// ── Handlers ──

#[require_permission("QMS", "create")]
pub async fn get_create(
    _path: SpecCreatePath,
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

    let product_svc = state.product_service();
    let products = product_svc
        .list(
            &service_ctx,
            &mut conn,
            abt_core::master_data::product::model::ProductQuery::default(),
            PageParams::new(1, 500),
        )
        .await?;

    let content = spec_create_page(&products.items);
    let page_html = admin_page(
        is_htmx,
        "新建检验规格",
        &claims,
        "quality",
        SpecCreatePath::PATH,
        "质量管理",
        Some(SpecListPath::PATH),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("QMS", "create")]
pub async fn create(
    _path: SpecCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SpecCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let inspection_type = abt_core::qms::enums::InspectionType::from_i16(form.inspection_type)
        .ok_or_else(|| {
            abt_core::shared::types::DomainError::Validation("无效检验类型".into())
        })?;

    let check_items: Vec<CheckItem> = if form.check_items_json.is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&form.check_items_json).unwrap_or_default()
    };

    let aql: rust_decimal::Decimal = form.sample_aql.parse().unwrap_or_default();

    let req = CreateInspectionSpecificationReq {
        product_id: form.product_id,
        inspection_type,
        check_items,
        sample_plan: SamplePlan {
            level: form.sample_level,
            aql,
            mode: form.sample_mode,
        },
    };

    let svc = state.inspection_specification_service();
    let _id = svc.create(&service_ctx, &mut conn, req).await?;

    Ok(
        axum::response::Response::builder()
            .header("HX-Redirect", SpecListPath::PATH)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
}

// ── Page rendering ──

fn spec_create_page(products: &[abt_core::master_data::product::model::Product]) -> Markup {
    html! {
        // ── Inline styles for radio group ──
        style { (PreEscaped(r#"
            .radio-group{display:flex;gap:var(--space-2);flex-wrap:wrap}
            .radio-option{display:flex;align-items:center;gap:6px;padding:8px 16px;border:1px solid var(--border);border-radius:var(--radius-sm);cursor:pointer;font-size:var(--text-sm);color:var(--fg-2);transition:all var(--motion-fast);background:var(--bg)}
            .radio-option:hover{border-color:var(--accent);color:var(--accent)}
            .radio-option.active{border-color:var(--accent);background:rgba(37,99,235,0.06);color:var(--accent);font-weight:600}
            .add-row-btn{display:flex;align-items:center;justify-content:center;gap:6px;width:100%;padding:10px;border:2px dashed var(--border);border-radius:var(--radius-sm);background:transparent;color:var(--fg-muted);font-size:var(--text-sm);cursor:pointer;transition:all var(--motion-fast)}
            .add-row-btn:hover{border-color:var(--accent);color:var(--accent)}
        "#)) }

        div {
            // ── Page Header ──
            div class="page-header" style="margin-bottom:var(--space-6)" {
                h1 class="page-title" { "新建检验规格" }
            }

            form id="spec-form" hx-post=(SpecCreatePath::PATH) hx-swap="none" {

                // ── Section 1: 基本信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::file_text_icon("w-[18px] h-[18px]"))
                        "基本信息"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        // 产品
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "产品"
                                span class="required" { "*" }
                            }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="product_id" required {
                                option value="" disabled selected { "请选择产品" }
                                @for p in products {
                                    option value=(p.product_id) { (p.product_code) " — " (p.pdt_name) }
                                }
                            }
                        }

                        // 检验类型 — radio group spanning 2 cols
                        div class="form-field" style="grid-column:span 2" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "检验类型"
                                span class="required" { "*" }
                            }
                            input type="hidden" name="inspection_type" id="inspection-type-input" value="1";
                            div class="radio-group" {
                                label class="radio-option active" data-value="1" onclick="specSelectInspectionType(this)" {
                                    input type="radio" name="inspection_type_radio" value="1" checked style="display:none";
                                    "IQC 来料检验"
                                }
                                label class="radio-option" data-value="2" onclick="specSelectInspectionType(this)" {
                                    input type="radio" name="inspection_type_radio" value="2" style="display:none";
                                    "IPQC 过程检验"
                                }
                                label class="radio-option" data-value="3" onclick="specSelectInspectionType(this)" {
                                    input type="radio" name="inspection_type_radio" value="3" style="display:none";
                                    "FQC 终检"
                                }
                                label class="radio-option" data-value="4" onclick="specSelectInspectionType(this)" {
                                    input type="radio" name="inspection_type_radio" value="4" style="display:none";
                                    "OQC 出货检"
                                }
                            }
                        }
                    }
                }

                // ── Section 2: 检验项目 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::clipboard_list_icon("w-[18px] h-[18px]"))
                        "检验项目"
                    }
                    div class="data-card-scroll" {
                        table class="data-table" id="check-items-table" {
                            thead {
                                tr {
                                    th style="width:40px" { "#" }
                                    th { "检验项目 " span class="required" { "*" } }
                                    th { "检验标准 " span class="required" { "*" } }
                                    th { "公差范围" }
                                    th { "检验方法 " span class="required" { "*" } }
                                    th style="width:40px" {}
                                }
                            }
                            tbody id="check-items-body" {
                                // Row 1: 外观检查
                                tr class="check-item-row" {
                                    td class="row-num" { "1" }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_item" value="外观检查"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_standard" value="目视无划痕"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_tolerance" value="无明显缺陷"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_method" value="目视检查"; }
                                    td { button type="button" class="btn-remove-row" title="删除行" { (icon::trash_icon("w-4 h-4")) } }
                                }
                                // Row 2: 尺寸测量
                                tr class="check-item-row" {
                                    td class="row-num" { "2" }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_item" value="尺寸测量"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_standard" value="图纸公差要求"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_tolerance" value="±0.05mm"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_method" value="游标卡尺"; }
                                    td { button type="button" class="btn-remove-row" title="删除行" { (icon::trash_icon("w-4 h-4")) } }
                                }
                                // Row 3: 电气性能
                                tr class="check-item-row" {
                                    td class="row-num" { "3" }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_item" value="电气性能"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_standard" value="额定电压电流"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_tolerance" value="±5%"; }
                                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_method" value="万用表"; }
                                    td { button type="button" class="btn-remove-row" title="删除行" { (icon::trash_icon("w-4 h-4")) } }
                                }
                            }
                        }
                    }
                    button type="button" class="add-row-btn" id="add-check-item-btn" {
                        (icon::plus_icon("w-4 h-4"))
                        "添加检验项目"
                    }
                }

                // ── Section 3: 抽样方案 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::sliders_icon("w-[18px] h-[18px]"))
                        "抽样方案"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验水平" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="sample_level" {
                                option value="I" { "Level I" }
                                option value="II" selected { "Level II" }
                                option value="III" { "Level III" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "AQL值" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="sample_aql" {
                                option value="0.25" { "0.25" }
                                option value="0.65" { "0.65" }
                                option value="1.0" selected { "1.0" }
                                option value="1.5" { "1.5" }
                                option value="2.5" { "2.5" }
                                option value="4.0" { "4.0" }
                            }
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "抽样模式" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="sample_mode" {
                                option value="normal" selected { "正常" }
                                option value="tightened" { "加严" }
                                option value="reduced" { "放宽" }
                            }
                        }
                    }
                }

                // ── Hidden field for check items JSON ──
                input type="hidden" name="check_items_json" id="check-items-json";

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(format!("{}?restore=true", SpecListPath::PATH)) { "取消" }
                    button type="button" class="btn btn-default" id="save-draft-btn" { "保存草稿" }
                    button type="submit" class="btn btn-primary" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "提交审核"
                    }
                }
            }
        }

        // ── Dynamic row + radio group JS ──
        script { (PreEscaped(r#"
(function() {
    var tbody = document.getElementById('check-items-body');
    var addBtn = document.getElementById('add-check-item-btn');
    var hiddenJson = document.getElementById('check-items-json');
    var form = document.getElementById('spec-form');

    function rowNumber(tr) {
        var rows = tbody.querySelectorAll('.check-item-row');
        for (var i = 0; i < rows.length; i++) {
            if (rows[i] === tr) return i + 1;
        }
        return 0;
    }

    function renumberRows() {
        var rows = tbody.querySelectorAll('.check-item-row');
        rows.forEach(function(row, idx) {
            var numCell = row.querySelector('.row-num');
            if (numCell) numCell.textContent = idx + 1;
        });
    }

    function createRow() {
        var tr = document.createElement('tr');
        tr.className = 'check-item-row';
        var num = tbody.querySelectorAll('.check-item-row').length + 1;
        tr.innerHTML =
            '<td class="row-num">' + num + '</td>' +
            '<td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_item" placeholder="检验项目"></td>' +
            '<td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_standard" placeholder="检验标准"></td>' +
            '<td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_tolerance" placeholder="公差范围"></td>' +
            '<td><input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="ci_method" placeholder="检验方法"></td>' +
            '<td><button type="button" class="btn-remove-row" title="删除行"><svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg></button></td>';
        return tr;
    }

    function removeRow(e) {
        var btn = e.target.closest('.btn-remove-row');
        if (btn) {
            btn.closest('tr').remove();
            renumberRows();
        }
    }

    addBtn.addEventListener('click', function() {
        tbody.appendChild(createRow());
    });

    tbody.addEventListener('click', removeRow);

    form.addEventListener('htmx:beforeRequest', function() {
        var rows = tbody.querySelectorAll('.check-item-row');
        var items = [];
        rows.forEach(function(row) {
            var item = row.querySelector('[name="ci_item"]').value.trim();
            var standard = row.querySelector('[name="ci_standard"]').value.trim();
            var tolerance = row.querySelector('[name="ci_tolerance"]').value.trim();
            var method = row.querySelector('[name="ci_method"]').value.trim();
            if (item || standard || tolerance || method) {
                items.push({
                    item: item,
                    standard: standard,
                    tolerance: tolerance,
                    method: method
                });
            }
        });
        hiddenJson.value = JSON.stringify(items);
    });
})();

// Radio group for inspection type
function specSelectInspectionType(el) {
    var group = el.closest('.radio-group');
    var options = group.querySelectorAll('.radio-option');
    options.forEach(function(opt) { opt.classList.remove('active'); });
    el.classList.add('active');
    document.getElementById('inspection-type-input').value = el.getAttribute('data-value');
}
"#)) }
    }
}
